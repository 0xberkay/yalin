//! Whole-value traversal for building trees or other representations.
//!
//! [`Value::walk`] streams every value under a node to a [`Visitor`] in
//! document order. Unlike path queries, a walk is strict about object keys:
//! a key that repeats within one object (compared after unescaping) fails the
//! walk with [`ErrorKind::DuplicateKey`].

use std::borrow::Cow;
use std::collections::HashSet;

use crate::tape::Tag;
use crate::{ErrorKind, JsonError, Value, strings};

/// Receives the events of [`Value::walk`].
///
/// Containers are reported as `begin_*`, their children, then `end_*` with the
/// number of children (members for an object). Inside an object every value is
/// preceded by its [`key`](Self::key).
pub trait Visitor {
    fn null(&mut self);
    fn bool(&mut self, value: bool);
    /// The number exactly as written in the input (RFC 8259 grammar, ASCII).
    fn number(&mut self, text: &str);
    fn string(&mut self, value: &str);
    fn key(&mut self, key: &str);
    fn begin_array(&mut self);
    fn end_array(&mut self, len: usize);
    fn begin_object(&mut self);
    fn end_object(&mut self, len: usize);
}

/// Objects with more keys than this switch from a linear duplicate scan to a hash set.
const LINEAR_KEYS: usize = 16;

struct Frame<'d> {
    /// Tape index one past the container's last descendant.
    end: usize,
    /// Children seen so far (members, for an object).
    len: usize,
    object: bool,
    /// Start of this object's keys in `keys` while the scan is linear.
    keys_from: usize,
    /// One bit per key, chosen by [`key_bit`]: a key whose bit is clear is new,
    /// so most keys skip the linear scan.
    bits: u64,
    /// Keys of a large object, once it outgrows the linear scan.
    set: Option<HashSet<Cow<'d, str>>>,
}

impl<'d> Value<'d> {
    /// Streams this value and everything under it to `visitor`; fails on the first
    /// duplicate key, after the visitor has seen the events before it.
    ///
    /// Runs in one pass over the parsed document with an explicit stack, so
    /// nesting depth is bounded by the parse [`Limits`](crate::Limits), not the
    /// native call stack.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) -> Result<(), JsonError> {
        let end = self.node().skip(self.idx);
        // Sized for typical nesting and object widths, so small documents never regrow them.
        let mut stack: Vec<Frame<'d>> = Vec::with_capacity(16);
        let mut keys: Vec<Cow<'d, str>> = Vec::with_capacity(64);
        // Unescaped string values, reused so that escapes cost no allocation.
        let mut scratch = Vec::new();
        let mut i = self.idx;
        while i < end {
            close_finished(&mut stack, &mut keys, i, visitor);
            let node = self.tape[i];
            if let Some(frame) = stack.last_mut() {
                if frame.object {
                    // Keys sit at even positions among an object's children.
                    let key = self.at(i).string_cow();
                    // Error offset: the key's opening quote.
                    check_unique(frame, &mut keys, key.clone(), node.start as usize - 1)?;
                    visitor.key(&key);
                    i += 1;
                }
                frame.len += 1;
            }
            let node = self.tape[i];
            match node.tag {
                Tag::Null => visitor.null(),
                Tag::True => visitor.bool(true),
                Tag::False => visitor.bool(false),
                Tag::Number => visitor.number(self.at(i).number_text()),
                Tag::String => visitor.string(crate::query::validated_str(&self.input[node.span()])),
                Tag::EscapedString => {
                    scratch.clear();
                    strings::unescape_into(&self.input[node.span()], &mut scratch);
                    visitor.string(crate::query::validated_str(&scratch));
                }
                Tag::Array | Tag::Object => {
                    let object = node.tag == Tag::Object;
                    if object { visitor.begin_object() } else { visitor.begin_array() }
                    stack.push(Frame { end: node.end as usize, len: 0, object, keys_from: keys.len(), bits: 0, set: None });
                }
            }
            i += 1;
        }
        close_finished(&mut stack, &mut keys, usize::MAX, visitor);
        Ok(())
    }

    /// String content of a string node, unescaped.
    fn string_cow(&self) -> Cow<'d, str> {
        let bytes = &self.input[self.node().span()];
        match self.node().tag {
            Tag::EscapedString => Cow::Owned(strings::unescape(bytes)),
            _ => Cow::Borrowed(crate::query::validated_str(bytes)),
        }
    }

    fn number_text(&self) -> &'d str {
        crate::query::validated_str(&self.input[self.node().span()])
    }
}

/// Ends every open container whose last descendant is before tape index `i`
/// (all of them for `usize::MAX`).
#[inline(always)]
fn close_finished<V: Visitor>(stack: &mut Vec<Frame<'_>>, keys: &mut Vec<Cow<'_, str>>, i: usize, visitor: &mut V) {
    while let Some(frame) = stack.last() {
        if frame.end > i {
            break;
        }
        let frame = stack.pop().unwrap_or_else(|| unreachable!());
        if frame.object {
            keys.truncate(frame.keys_from);
            visitor.end_object(frame.len);
        } else {
            visitor.end_array(frame.len);
        }
    }
}

/// A cheap 1-of-64 fingerprint of a key: its length and first and last bytes.
fn key_bit(key: &str) -> u64 {
    let b = key.as_bytes();
    let h = match b {
        [] => 0,
        [first, .., last] => b.len() * 7 + *first as usize + 3 * *last as usize,
        [only] => 7 + 4 * *only as usize,
    };
    1 << (h & 63)
}

fn check_unique<'d>(
    frame: &mut Frame<'d>,
    keys: &mut Vec<Cow<'d, str>>,
    key: Cow<'d, str>,
    offset: usize,
) -> Result<(), JsonError> {
    let duplicate = match &mut frame.set {
        Some(set) => !set.insert(key),
        None if frame.len < LINEAR_KEYS => {
            let bit = key_bit(&key);
            let seen = frame.bits & bit != 0 && keys[frame.keys_from..].contains(&key);
            frame.bits |= bit;
            keys.push(key);
            seen
        }
        None => {
            let mut set: HashSet<_> = keys.drain(frame.keys_from..).collect();
            let seen = !set.insert(key);
            frame.set = Some(set);
            seen
        }
    };
    if duplicate { Err(JsonError::new(ErrorKind::DuplicateKey, offset)) } else { Ok(()) }
}
