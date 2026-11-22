//! Wire formats shared with the Kotlin side (`YalinNative.kt` / `YalinProjection.kt`)
//! and the Swift side (`apple/Sources/Yalin/YalinProjection.swift`).
//! Keep all sides in sync. All integers are little-endian.
//!
//! **Paths in** (`ByteArray`): repeated `[u32 len][len bytes UTF-8 path]`.
//!
//! **Results out** (`ByteArray`): `[i64 status]` then, if `status == 0`, one
//! entry per path: `[u8 tag][payload]`
//!
//! | tag | meaning        | payload                          |
//! |-----|----------------|----------------------------------|
//! | 0   | missing        | –                                |
//! | 1   | null           | –                                |
//! | 2   | false          | –                                |
//! | 3   | true           | –                                |
//! | 4   | integer        | `i64`                            |
//! | 5   | number         | `f64` (not an `i64`, finite)     |
//! | 6   | big number     | `[u32 len][ASCII]` (overflows f64) |
//! | 7   | string         | `[u32 len][UTF-8]`               |
//! | 8   | array          | –                                |
//! | 9   | object         | –                                |
//! | 10  | invalid path   | `u32` byte position in the path  |
//! | 11  | type mismatch  | `u8 expected kind`, `u8 found kind` (see [`kind_code`]) |
//!
//! A non-zero status is a parse error code (see [`parse_error_code`]).
//!
//! The whole-tree format is in [`crate::tree`].
//!
//! **Parse error code** (`i64`, always negative):
//! `-(((offset << 8) | kind) + 1)`, kind numbering as in `YalinParseError.kt`.

use yalin_core::{Document, ErrorKind, JsonError, QueryError, Value, ValueKind};

pub const MISSING: u8 = 0;
pub const NULL: u8 = 1;
pub const FALSE: u8 = 2;
pub const TRUE: u8 = 3;
pub const INT: u8 = 4;
pub const DOUBLE: u8 = 5;
pub const BIG_NUMBER: u8 = 6;
pub const STRING: u8 = 7;
pub const ARRAY: u8 = 8;
pub const OBJECT: u8 = 9;
pub const INVALID_PATH: u8 = 10;
pub const TYPE_MISMATCH: u8 = 11;

/// Error for a malformed request from a binding (a bug, not bad JSON).
#[derive(Debug)]
pub struct ProtocolError(pub &'static str);

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ProtocolError {}

pub fn parse_error_code(e: &JsonError) -> i64 {
    let kind: i64 = match e.kind() {
        ErrorKind::InputTooLarge => 0,
        ErrorKind::UnexpectedEof => 1,
        ErrorKind::UnexpectedCharacter => 2,
        ErrorKind::UnterminatedString => 3,
        ErrorKind::ControlCharacterInString => 4,
        ErrorKind::InvalidEscape => 5,
        ErrorKind::InvalidSurrogate => 6,
        ErrorKind::InvalidUtf8 => 7,
        ErrorKind::InvalidLiteral => 8,
        ErrorKind::InvalidNumber => 9,
        ErrorKind::DepthLimitExceeded => 10,
        ErrorKind::TrailingCharacters => 11,
        ErrorKind::DuplicateKey => 12,
        _ => 255,
    };
    -((((e.offset() as i64) << 8) | kind) + 1)
}

/// `ValueKind` as the ordinal of Kotlin's `YalinKind` (0 is `MISSING`).
pub fn kind_code(kind: ValueKind) -> u8 {
    match kind {
        ValueKind::Null => 1,
        ValueKind::Bool => 2,
        ValueKind::Number => 3,
        ValueKind::String => 4,
        ValueKind::Array => 5,
        ValueKind::Object => 6,
    }
}

pub fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    // Spans come from an input capped at u32::MAX bytes.
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Evaluates every framed path against `doc`; status 0 followed by entries.
pub fn query<B: AsRef<[u8]>>(doc: &Document<B>, paths: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::with_capacity(8 + paths.len());
    out.extend_from_slice(&0i64.to_le_bytes());
    let mut rest = paths;
    while !rest.is_empty() {
        let (len, tail) = rest.split_first_chunk::<4>().ok_or(ProtocolError("truncated path length"))?;
        let len = u32::from_le_bytes(*len) as usize;
        if tail.len() < len {
            return Err(ProtocolError("truncated path"));
        }
        let (path, tail) = tail.split_at(len);
        rest = tail;
        match std::str::from_utf8(path) {
            Ok(path) => encode(&mut out, doc.get(path)),
            Err(e) => encode(&mut out, Err(QueryError::InvalidPath { position: e.valid_up_to() })),
        }
    }
    Ok(out)
}

/// Status-only result for a failed parse.
pub fn parse_failure(e: &JsonError) -> Vec<u8> {
    parse_error_code(e).to_le_bytes().to_vec()
}

fn encode(out: &mut Vec<u8>, result: Result<Value<'_>, QueryError>) {
    let v = match result {
        Ok(v) => v,
        Err(QueryError::Missing) => return out.push(MISSING),
        Err(QueryError::InvalidPath { position }) => {
            out.push(INVALID_PATH);
            out.extend_from_slice(&u32::try_from(position).unwrap_or(u32::MAX).to_le_bytes());
            return;
        }
        Err(QueryError::TypeMismatch { expected, found }) => {
            return out.extend_from_slice(&[TYPE_MISMATCH, kind_code(expected), kind_code(found)]);
        }
        // Lookups never produce this; conversions are done below.
        Err(QueryError::NumberNotRepresentable) => return out.push(MISSING),
    };
    match v.kind() {
        ValueKind::Null => out.push(NULL),
        ValueKind::Bool => out.push(if v.as_bool() == Ok(true) { TRUE } else { FALSE }),
        ValueKind::Number => {
            if let Ok(i) = v.as_i64() {
                out.push(INT);
                out.extend_from_slice(&i.to_le_bytes());
            } else if let Ok(f) = v.as_f64() {
                out.push(DOUBLE);
                out.extend_from_slice(&f.to_le_bytes());
            } else {
                out.push(BIG_NUMBER);
                put_bytes(out, v.as_number_str().unwrap_or_default().as_bytes());
            }
        }
        ValueKind::String => {
            out.push(STRING);
            put_bytes(out, v.as_str().unwrap_or_default().as_bytes());
        }
        ValueKind::Array => out.push(ARRAY),
        ValueKind::Object => out.push(OBJECT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(paths: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for p in paths {
            put_bytes(&mut out, p.as_bytes());
        }
        out
    }

    #[test]
    fn encodes_every_outcome() {
        let doc = Document::parse(
            br#"{"s":"h\u00e9","i":-7,"f":1.5,"b":1e400,"t":true,"n":null,"a":[],"o":{}}"#.as_slice(),
        )
        .unwrap();
        let out = query(&doc, &frame(&["s", "i", "f", "b", "t", "n", "a", "o", "x", "a.k", "s..", "o.zz"])).unwrap();
        let mut want = 0i64.to_le_bytes().to_vec();
        want.extend([STRING, 3, 0, 0, 0, b'h', 0xC3, 0xA9]);
        want.push(INT);
        want.extend((-7i64).to_le_bytes());
        want.push(DOUBLE);
        want.extend(1.5f64.to_le_bytes());
        want.extend([BIG_NUMBER, 5, 0, 0, 0]);
        want.extend(b"1e400");
        want.extend([TRUE, NULL, ARRAY, OBJECT, MISSING]);
        want.extend([TYPE_MISMATCH, 6, 5]);
        want.extend([INVALID_PATH, 2, 0, 0, 0]);
        want.push(MISSING);
        assert_eq!(out, want);
    }

    #[test]
    fn empty_request_and_bad_utf8_path() {
        let doc = Document::parse(b"{}".as_slice()).unwrap();
        assert_eq!(query(&doc, &[]).unwrap(), 0i64.to_le_bytes());
        let out = query(&doc, &[2, 0, 0, 0, b'a', 0xFF]).unwrap();
        assert_eq!(&out[8..], &[INVALID_PATH, 1, 0, 0, 0]);
    }

    #[test]
    fn rejects_bad_framing() {
        let doc = Document::parse(b"{}".as_slice()).unwrap();
        assert!(query(&doc, &[1, 0]).is_err());
        assert!(query(&doc, &[5, 0, 0, 0, b'a']).is_err());
    }

    #[test]
    fn parse_error_code_round_trips() {
        let e = Document::parse(b"[1,  ]".as_slice()).err().unwrap();
        let code = parse_error_code(&e);
        assert!(code < 0);
        let packed = -(code + 1);
        assert_eq!((packed & 0xFF, packed >> 8), (2, 5));
        assert_eq!(parse_failure(&e), code.to_le_bytes());
    }
}
