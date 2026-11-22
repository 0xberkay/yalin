//! Selective reads over a fully validated document.
//!
//! # Path syntax
//!
//! A path is a sequence of segments, evaluated from the value it is applied to:
//!
//! - `name` — object member, matched against the **decoded** key (so `"a"` matches `a`).
//!   The first segment has no leading dot; later member segments are preceded by `.`.
//! - `[N]` — zero-based array element; `N` is decimal without leading zeros.
//! - the empty path `""` selects the value itself.
//!
//! Examples: `user.name`, `items[3].price`, `[0].id`, `matrix[1][2]`.
//!
//! Limitation: member names containing `.`, `[` or `]`, and the empty name,
//! cannot be addressed.
//!
//! # Semantics
//!
//! - The whole document is validated before any query: a value is never
//!   returned from malformed JSON.
//! - Duplicate keys: the **first** occurrence wins.
//! - Outcomes are distinct: [`QueryError::Missing`] (no such member or index),
//!   a JSON `null` (an `Ok` value of kind [`ValueKind::Null`]),
//!   [`QueryError::TypeMismatch`] (e.g. `as_str` on a number, or indexing an object),
//!   and [`QueryError::InvalidPath`].
//! - `as_str` borrows from the input when the string has no escapes and
//!   allocates only to decode escapes.
//!
//! A [`Document`] is immutable after parsing, so it is `Send + Sync` whenever
//! its buffer is, and may be queried from several threads at once.

use std::borrow::Cow;
use std::fmt;

use crate::tape::{Node, Tag, Tape};
use crate::{JsonError, Limits, parser, strings};

/// JSON value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Number => "number",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
        })
    }
}

/// Why a query did not produce the requested value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// The path string is malformed; `position` is a byte offset into it.
    InvalidPath { position: usize },
    /// The member or index does not exist.
    Missing,
    /// The value (or an intermediate value on the path) has another type.
    TypeMismatch { expected: ValueKind, found: ValueKind },
    /// A number that is valid JSON but does not fit the requested type
    /// (fractional for an integer, out of range, or non-finite as `f64`).
    NumberNotRepresentable,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { position } => write!(f, "invalid path at position {position}"),
            Self::Missing => f.write_str("value not found"),
            Self::TypeMismatch { expected, found } => write!(f, "expected {expected}, found {found}"),
            Self::NumberNotRepresentable => f.write_str("number not representable in requested type"),
        }
    }
}

impl std::error::Error for QueryError {}

/// A validated JSON document over a buffer it borrows (`&[u8]`) or owns (`Vec<u8>`).
pub struct Document<B: AsRef<[u8]>> {
    input: B,
    tape: Tape,
}

impl<B: AsRef<[u8]>> Document<B> {
    /// Fully validates `input` with default [`Limits`].
    pub fn parse(input: B) -> Result<Self, JsonError> {
        Self::parse_with_limits(input, &Limits::default())
    }

    pub fn parse_with_limits(input: B, limits: &Limits) -> Result<Self, JsonError> {
        let tape = parser::parse(input.as_ref(), limits)?;
        Ok(Self { input, tape })
    }

    pub fn root(&self) -> Value<'_> {
        Value { input: self.input.as_ref(), tape: &self.tape, idx: 0 }
    }

    /// Looks up `path` from the root. See the [module docs](self) for syntax.
    pub fn get(&self, path: &str) -> Result<Value<'_>, QueryError> {
        self.root().get(path)
    }

    /// Looks up several paths in one call; results are in `paths` order.
    pub fn project(&self, paths: &[&str]) -> Vec<Result<Value<'_>, QueryError>> {
        paths.iter().map(|p| self.get(p)).collect()
    }
}

/// A value inside a [`Document`]. Cheap to copy.
#[derive(Clone, Copy)]
pub struct Value<'d> {
    pub(crate) input: &'d [u8],
    pub(crate) tape: &'d Tape,
    pub(crate) idx: usize,
}

impl<'d> Value<'d> {
    pub(crate) fn node(&self) -> Node {
        self.tape.node(self.idx)
    }

    pub(crate) fn at(&self, idx: usize) -> Self {
        Self { idx, ..*self }
    }

    fn bytes(&self) -> &'d [u8] {
        &self.input[self.node().span()]
    }

    pub fn kind(&self) -> ValueKind {
        match self.node().tag {
            Tag::Null => ValueKind::Null,
            Tag::True | Tag::False => ValueKind::Bool,
            Tag::Number => ValueKind::Number,
            Tag::String | Tag::EscapedString => ValueKind::String,
            Tag::Array => ValueKind::Array,
            Tag::Object => ValueKind::Object,
        }
    }

    fn expect(&self, expected: ValueKind) -> Result<(), QueryError> {
        let found = self.kind();
        if found == expected { Ok(()) } else { Err(QueryError::TypeMismatch { expected, found }) }
    }

    pub fn is_null(&self) -> bool {
        self.node().tag == Tag::Null
    }

    pub fn as_bool(&self) -> Result<bool, QueryError> {
        self.expect(ValueKind::Bool)?;
        Ok(self.node().tag == Tag::True)
    }

    pub fn as_str(&self) -> Result<Cow<'d, str>, QueryError> {
        self.expect(ValueKind::String)?;
        let bytes = self.bytes();
        Ok(match self.node().tag {
            Tag::EscapedString => Cow::Owned(strings::unescape(bytes)),
            _ => Cow::Borrowed(validated_str(bytes)),
        })
    }

    /// The number exactly as written in the input.
    pub fn as_number_str(&self) -> Result<&'d str, QueryError> {
        self.expect(ValueKind::Number)?;
        Ok(validated_str(self.bytes()))
    }

    /// Integer value; fails for fractions/exponents (`1.0`, `1e2`) and out-of-range values.
    pub fn as_i64(&self) -> Result<i64, QueryError> {
        self.as_number_str()?.parse().map_err(|_| QueryError::NumberNotRepresentable)
    }

    /// Nearest `f64` (correctly rounded); fails if the value overflows to infinity.
    pub fn as_f64(&self) -> Result<f64, QueryError> {
        let v: f64 = self.as_number_str()?.parse().map_err(|_| QueryError::NumberNotRepresentable)?;
        if v.is_finite() { Ok(v) } else { Err(QueryError::NumberNotRepresentable) }
    }

    /// Looks up `path` relative to this value. See the [module docs](self) for syntax.
    pub fn get(&self, path: &str) -> Result<Value<'d>, QueryError> {
        // Syntax first, so a malformed path is always reported as such
        // rather than as whatever the partial walk hit.
        Segments::new(path).try_for_each(|s| s.map(drop))?;
        let mut cur = *self;
        for segment in Segments::new(path) {
            cur = match segment? {
                Segment::Key(key) => cur.member(key)?,
                Segment::Index(n) => cur.element(n)?,
            };
        }
        Ok(cur)
    }

    fn member(&self, key: &str) -> Result<Value<'d>, QueryError> {
        self.expect(ValueKind::Object)?;
        let end = self.node().end as usize;
        let mut i = self.idx + 1;
        while i < end {
            if self.at(i).key_eq(key) {
                return Ok(self.at(i + 1));
            }
            i = self.tape.node(i + 1).skip(i + 1);
        }
        Err(QueryError::Missing)
    }

    fn element(&self, n: usize) -> Result<Value<'d>, QueryError> {
        self.expect(ValueKind::Array)?;
        let end = self.node().end as usize;
        let mut i = self.idx + 1;
        for _ in 0..n {
            if i >= end {
                break;
            }
            i = self.tape.node(i).skip(i);
        }
        if i < end { Ok(self.at(i)) } else { Err(QueryError::Missing) }
    }

    /// `self` must be a key node.
    fn key_eq(&self, key: &str) -> bool {
        let raw = self.bytes();
        match self.node().tag {
            Tag::EscapedString => strings::unescape(raw) == key,
            _ => raw == key.as_bytes(),
        }
    }
}

/// A string or number span of a parsed document, or an unescaped string, as `&str`.
pub(crate) fn validated_str(span: &[u8]) -> &str {
    debug_assert!(std::str::from_utf8(span).is_ok());
    // SAFETY: the parser validated the whole input as UTF-8, and string and
    // number spans start and end next to ASCII bytes (quotes, delimiters), so
    // they never split a multi-byte sequence. Unescaping such a span keeps the
    // bytes between escapes and writes each escape as a whole `char`.
    unsafe { std::str::from_utf8_unchecked(span) }
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node = self.node();
        let text = match node.tag {
            Tag::Array | Tag::Object => &self.input[node.start as usize..=node.start as usize],
            Tag::Null => b"null".as_slice(),
            Tag::True => b"true",
            Tag::False => b"false",
            _ => self.bytes(),
        };
        write!(f, "Value({}: {})", self.kind(), String::from_utf8_lossy(text))
    }
}

enum Segment<'p> {
    Key(&'p str),
    Index(usize),
}

/// Parses path segments without allocating.
struct Segments<'p> {
    path: &'p str,
    pos: usize,
}

impl<'p> Segments<'p> {
    fn new(path: &'p str) -> Self {
        Self { path, pos: 0 }
    }

    fn invalid<T>(&mut self, position: usize) -> Option<Result<T, QueryError>> {
        self.pos = self.path.len(); // stop iteration
        Some(Err(QueryError::InvalidPath { position }))
    }
}

impl<'p> Iterator for Segments<'p> {
    type Item = Result<Segment<'p>, QueryError>;

    fn next(&mut self) -> Option<Self::Item> {
        let b = self.path.as_bytes();
        let start = self.pos;
        if start >= b.len() {
            return None;
        }
        if b[start] == b'[' {
            let digits = start + 1;
            let mut i = digits;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == digits || (b[digits] == b'0' && i > digits + 1) || b.get(i) != Some(&b']') {
                return self.invalid(i.min(b.len()));
            }
            let Ok(n) = self.path[digits..i].parse() else {
                return self.invalid(digits);
            };
            self.pos = i + 1;
            return Some(Ok(Segment::Index(n)));
        }
        let mut i = start;
        if start > 0 {
            if b[start] != b'.' {
                return self.invalid(start);
            }
            i += 1;
        }
        let name = i;
        while i < b.len() && !matches!(b[i], b'.' | b'[' | b']') {
            i += 1;
        }
        if i == name || b.get(i) == Some(&b']') {
            return self.invalid(i);
        }
        self.pos = i;
        Some(Ok(Segment::Key(&self.path[name..i])))
    }
}
