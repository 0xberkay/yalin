use std::fmt;

/// Why a document was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Input exceeds [`Limits::max_input_len`](crate::Limits) (hard cap: `u32::MAX` bytes).
    InputTooLarge,
    /// Input ended where more JSON was required (includes empty input).
    UnexpectedEof,
    /// A byte that is not allowed at this position by the JSON grammar.
    UnexpectedCharacter,
    /// A string was opened but never closed.
    UnterminatedString,
    /// Raw byte `< 0x20` inside a string.
    ControlCharacterInString,
    /// Unknown escape (`\x`) or malformed `\uXXXX`.
    InvalidEscape,
    /// `\uXXXX` escape forming a lone or mismatched UTF-16 surrogate.
    InvalidSurrogate,
    /// String bytes are not valid UTF-8.
    InvalidUtf8,
    /// Something starting like `true` / `false` / `null` that is not one.
    InvalidLiteral,
    /// Number does not follow the RFC 8259 grammar.
    InvalidNumber,
    /// Nesting deeper than [`Limits::max_depth`](crate::Limits).
    DepthLimitExceeded,
    /// Non-whitespace content after the root value.
    TrailingCharacters,
    /// An object key repeats (after unescaping). Only reported by
    /// [`Value::walk`](crate::Value::walk); path queries take the first occurrence.
    DuplicateKey,
}

impl ErrorKind {
    fn message(self) -> &'static str {
        match self {
            Self::InputTooLarge => "input too large",
            Self::UnexpectedEof => "unexpected end of input",
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnterminatedString => "unterminated string",
            Self::ControlCharacterInString => "control character in string",
            Self::InvalidEscape => "invalid escape sequence",
            Self::InvalidSurrogate => "invalid UTF-16 surrogate in \\u escape",
            Self::InvalidUtf8 => "invalid UTF-8",
            Self::InvalidLiteral => "invalid literal",
            Self::InvalidNumber => "invalid number",
            Self::DepthLimitExceeded => "nesting depth limit exceeded",
            Self::TrailingCharacters => "trailing characters after JSON value",
            Self::DuplicateKey => "duplicate object key",
        }
    }
}

/// A JSON syntax or resource-limit error, with the byte offset where it was detected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonError {
    kind: ErrorKind,
    offset: usize,
}

impl JsonError {
    pub(crate) fn new(kind: ErrorKind, offset: usize) -> Self {
        Self { kind, offset }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Byte offset into the input.
    pub fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.kind.message(), self.offset)
    }
}

impl std::error::Error for JsonError {}
