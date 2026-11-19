//! Compact, read-only document representation.
//!
//! Values are stored in document order. A container node is followed by its
//! children; an object's children alternate key (`String`/`EscapedString`) and
//! value. Strings and numbers are spans into the original input, decoded only
//! on demand.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
