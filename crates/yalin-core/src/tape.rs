//! Compact, read-only document representation.
//!
//! Values are stored in document order. A container node is followed by its
//! children; an object's children alternate key (`String`/`EscapedString`) and
//! value. Strings and numbers are spans into the original input, decoded only
//! on demand.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Tag {
    Null,
    True,
    False,
    Number,
    /// String content without escapes: span is directly usable as `&str`.
    String,
    /// String content containing escapes: must be unescaped.
    EscapedString,
    Array,
    Object,
}

/// Field meaning depends on `tag`:
/// - `String` / `EscapedString` / `Number`: `start..end` is the byte span
///   (string content excludes the quotes).
/// - `Array` / `Object`: `start` is the opening bracket offset, `end` is the
///   tape index one past the container's last descendant.
/// - literals: `start` is the byte offset, `end` unused.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Node {
    pub tag: Tag,
    pub start: u32,
    pub end: u32,
}

impl Node {
    /// Tape index of the node after this one and all its descendants.
    pub fn skip(&self, idx: usize) -> usize {
        match self.tag {
            Tag::Array | Tag::Object => self.end as usize,
            _ => idx + 1,
        }
    }

    pub fn span(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

const TAGS: [Tag; 8] =
    [Tag::Null, Tag::True, Tag::False, Tag::Number, Tag::String, Tag::EscapedString, Tag::Array, Tag::Object];
const START_BITS: u32 = 29;
const START_MASK: u32 = (1 << START_BITS) - 1;

#[derive(Clone, Copy)]
struct CompactNode {
    packed: u32,
    end: u32,
}

impl From<Node> for CompactNode {
    fn from(node: Node) -> Self {
        debug_assert!(node.start <= START_MASK);
        Self { packed: (node.tag as u32) << START_BITS | node.start, end: node.end }
    }
}

impl From<CompactNode> for Node {
    fn from(node: CompactNode) -> Self {
        Self {
            tag: TAGS[(node.packed >> START_BITS) as usize],
            start: node.packed & START_MASK,
            end: node.end,
        }
    }
}

const _: () = assert!(size_of::<CompactNode>() == 8);

/// Small inputs use eight-byte nodes. Larger inputs keep full-width offsets.
enum Storage {
    Compact(Vec<CompactNode>),
    Wide(Vec<Node>),
}

pub(crate) struct Tape(Storage);

/// Keep at most one compact allocation of up to 4 MiB per thread.
const MAX_SPARE_NODES: usize = (4 << 20) / size_of::<CompactNode>();

std::thread_local! {
    static SPARE: std::cell::Cell<Vec<CompactNode>> = const { std::cell::Cell::new(Vec::new()) };
}

impl Tape {
    pub fn new(input_len: usize, capacity: usize) -> Self {
        if input_len > START_MASK as usize {
            return Self(Storage::Wide(Vec::with_capacity(capacity)));
        }
        let mut nodes = SPARE.try_with(|spare| spare.take()).unwrap_or_default();
        nodes.reserve(capacity);
        Self(Storage::Compact(nodes))
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            Storage::Compact(nodes) => nodes.len(),
            Storage::Wide(nodes) => nodes.len(),
        }
    }

    pub fn node(&self, index: usize) -> Node {
        match &self.0 {
            Storage::Compact(nodes) => nodes[index].into(),
            Storage::Wide(nodes) => nodes[index],
        }
    }

    pub fn push(&mut self, node: Node) {
        match &mut self.0 {
            Storage::Compact(nodes) => nodes.push(node.into()),
            Storage::Wide(nodes) => nodes.push(node),
        }
    }

    pub fn close(&mut self, index: usize) {
        let end = self.len() as u32;
        match &mut self.0 {
            Storage::Compact(nodes) => nodes[index].end = end,
            Storage::Wide(nodes) => nodes[index].end = end,
        }
    }
}

impl Drop for Tape {
    fn drop(&mut self) {
        let Storage::Compact(nodes) = &mut self.0 else { return };
        if nodes.capacity() > MAX_SPARE_NODES {
            return;
        }
        let mut nodes = std::mem::take(nodes);
        nodes.clear();
        let _ = SPARE.try_with(|spare| {
            let previous = spare.take();
            spare.set(if nodes.capacity() >= previous.capacity() { nodes } else { previous });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_width_preserves_spans_and_container_ends() {
        for start in [START_MASK, START_MASK + 1, u32::MAX - 1] {
            let mut tape = Tape::new(start as usize, 3);
            assert_eq!(matches!(tape.0, Storage::Compact(_)), start == START_MASK);
            tape.push(Node { tag: Tag::Array, start: start - 1, end: 0 });
            tape.push(Node { tag: Tag::EscapedString, start, end: start + 1 });
            tape.close(0);
            assert_eq!(tape.node(0).skip(0), 2);
            assert_eq!(tape.node(1).tag, Tag::EscapedString);
            assert_eq!(tape.node(1).span(), start as usize..(start + 1) as usize);
        }
    }
}
