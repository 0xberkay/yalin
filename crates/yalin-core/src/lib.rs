//! Yalın core: a fully validating RFC 8259 JSON parser built for selective reads.
//!
//! Parsing runs in two stages — a structural scanner (stage 1) and a grammar,
//! UTF-8, escape and number validator that builds a compact tape (stage 2).
//! Values are decoded only when queried, or streamed in full with
//! [`Value::walk`] to build another representation.
//!
//! ```
//! use yalin_core::{Document, QueryError};
//!
//! let doc = Document::parse(br#"{"user":{"name":"Ada","age":36,"email":null}}"#.as_slice())?;
//! assert_eq!(doc.get("user.name")?.as_str()?, "Ada");
//! assert_eq!(doc.get("user.age")?.as_i64()?, 36);
//! assert!(doc.get("user.email")?.is_null());
//! assert_eq!(doc.get("user.phone").unwrap_err(), QueryError::Missing);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod number;
mod parser;
mod query;
mod scanner;
mod strings;
mod tape;
mod utf8;
mod walk;

#[cfg(test)]
mod tests;

pub use error::{ErrorKind, JsonError};
pub use query::{Document, QueryError, Value, ValueKind};
pub use walk::Visitor;

/// Resource guards applied while parsing.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Maximum array/object nesting depth. Default: 1024.
    pub max_depth: usize,
    /// Maximum input size in bytes; always capped at `u32::MAX`. Default: `u32::MAX`.
    pub max_input_len: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_depth: 1024, max_input_len: u32::MAX as usize }
    }
}

/// Checks that `input` is a single valid JSON value, with default [`Limits`].
pub fn validate(input: &[u8]) -> Result<(), JsonError> {
    parser::parse(input, &Limits::default()).map(drop)
}
