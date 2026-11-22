//! JSON string content validation and unescaping.
//!
//! "Content" is the bytes strictly between the quotes.

use crate::{ErrorKind, JsonError};

/// Validates the escapes in string content and returns whether there are any.
/// `base` is the content's offset in the input, for error positions.
///
/// Everything else about strings is checked before stage 2 sees them: UTF-8 over
/// the whole input ([`crate::parser`]) and control bytes by the scanner's masks.
/// Stage 2 only calls this for strings the scanner saw a backslash in, so plain
/// strings cost nothing here.
pub(crate) fn validate_escapes(content: &[u8], base: usize) -> Result<bool, JsonError> {
    let Some(mut i) = find_backslash(content, 0) else {
        return Ok(false);
    };
    loop {
        i = validate_escape(content, i, base)?;
        match find_backslash(content, i) {
            Some(next) => i = next,
            None => return Ok(true),
        }
    }
}

/// Index of the first `\\` at or after `from`.
#[inline]
fn find_backslash(content: &[u8], mut from: usize) -> Option<usize> {
    const ONES: u64 = u64::from_ne_bytes([0x01; 8]);
    const HIGH: u64 = u64::from_ne_bytes([0x80; 8]);
    const BACKSLASH: u64 = u64::from_ne_bytes([b'\\'; 8]);
    while let Some(chunk) = content.get(from..from + 8) {
        let x = u64::from_le_bytes(chunk.try_into().unwrap()) ^ BACKSLASH;
        // Zero-byte test: exact for the lowest zero byte, which is all we use.
        let zero = x.wrapping_sub(ONES) & !x & HIGH;
        if zero != 0 {
            return Some(from + (zero.trailing_zeros() / 8) as usize);
        }
        from += 8;
    }
    content.get(from..)?.iter().position(|&b| b == b'\\').map(|k| from + k)
}

/// Validates the escape starting at `content[i] == b'\\'`; returns the index after it.
fn validate_escape(content: &[u8], i: usize, base: usize) -> Result<usize, JsonError> {
    let invalid = |kind| JsonError::new(kind, base + i);
    match content.get(i + 1) {
        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => Ok(i + 2),
        Some(b'u') => {
            let unit = hex4(content, i + 2).ok_or(invalid(ErrorKind::InvalidEscape))?;
            match unit {
                0xD800..=0xDBFF => {
                    if content.get(i + 6..i + 8) != Some(b"\\u") {
                        return Err(invalid(ErrorKind::InvalidSurrogate));
                    }
                    let low = hex4(content, i + 8).ok_or(invalid(ErrorKind::InvalidEscape))?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err(invalid(ErrorKind::InvalidSurrogate));
                    }
                    Ok(i + 12)
                }
                0xDC00..=0xDFFF => Err(invalid(ErrorKind::InvalidSurrogate)),
                _ => Ok(i + 6),
            }
        }
        _ => Err(invalid(ErrorKind::InvalidEscape)),
    }
}

/// Exactly four hex digits at `at` (no sign, unlike `u16::from_str_radix`).
fn hex4(content: &[u8], at: usize) -> Option<u16> {
    content.get(at..at + 4)?.iter().try_fold(0u16, |acc, &b| {
        let d = (b as char).to_digit(16)?;
        Some(acc << 4 | d as u16)
    })
}

/// Decodes escapes. `content` must have passed [`validate_escapes`].
pub(crate) fn unescape(content: &[u8]) -> String {
    let mut out = Vec::with_capacity(content.len());
    unescape_into(content, &mut out);
    // Validated input splits only at ASCII '\', so this cannot fail; stay panic-free regardless.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// Appends the decoded `content` to `out`. `content` must have passed
/// [`validate_escapes`] and be valid UTF-8; then so are the appended bytes.
pub(crate) fn unescape_into(content: &[u8], out: &mut Vec<u8>) {
    out.reserve(content.len());
    let mut i = 0;
    while i < content.len() {
        let Some(k) = find_backslash(content, i) else {
            out.extend_from_slice(&content[i..]);
            return;
        };
        out.extend_from_slice(&content[i..k]);
        i = k;
        let decoded = match content[i + 1] {
            b'b' => '\u{8}',
            b'f' => '\u{C}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let hi = hex4(content, i + 2).unwrap_or(0xFFFD) as u32;
                let cp = if (0xD800..=0xDBFF).contains(&hi) {
                    let lo = hex4(content, i + 8).unwrap_or(0xDC00) as u32;
                    i += 6;
                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                } else {
                    hi
                };
                i += 4;
                char::from_u32(cp).unwrap_or('\u{FFFD}')
            }
            other => other as char, // '"', '\\', '/'
        };
        i += 2;
        let mut buf = [0u8; 4];
        out.extend_from_slice(decoded.encode_utf8(&mut buf).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(s: &[u8]) -> ErrorKind {
        validate_escapes(s, 0).unwrap_err().kind()
    }

    #[test]
    fn validates_escapes_and_surrogates() {
        assert_eq!(validate_escapes(b"plain", 0), Ok(false));
        assert_eq!(validate_escapes("h\u{e9}llo".as_bytes(), 0), Ok(false));
        assert_eq!(validate_escapes(br#"\"\\\/\b\f\n\r\t\u00e9"#, 0), Ok(true));
        assert_eq!(validate_escapes(br"\uD83D\uDE00", 0), Ok(true));
        assert_eq!(kind(br"\uD800"), ErrorKind::InvalidSurrogate);
        assert_eq!(kind(br"\uD800\u0041"), ErrorKind::InvalidSurrogate);
        assert_eq!(kind(br"\uDE00"), ErrorKind::InvalidSurrogate);
        assert_eq!(kind(br"\u12"), ErrorKind::InvalidEscape);
        assert_eq!(kind(br"\u+123"), ErrorKind::InvalidEscape);
        assert_eq!(kind(br"\x"), ErrorKind::InvalidEscape);
        assert_eq!(validate_escapes(br"ok\q", 10).unwrap_err().offset(), 12);
    }

    #[test]
    fn finds_backslash_in_any_lane() {
        for len in 0..20 {
            for pos in 0..len {
                let mut input = vec![b'x'; len];
                input[pos] = b'\\';
                assert_eq!(find_backslash(&input, 0), Some(pos));
                assert_eq!(find_backslash(&input, pos + 1), None);
                // The escape is either valid (`\n`) or cut off at the end.
                if pos + 1 < len {
                    input[pos + 1] = b'n';
                    assert_eq!(validate_escapes(&input, 0), Ok(true));
                } else {
                    assert_eq!(kind(&input), ErrorKind::InvalidEscape);
                }
            }
            assert_eq!(find_backslash(&vec![b'x'; len], 0), None);
        }
    }

    #[test]
    fn unescapes() {
        assert_eq!(
            unescape(br#"a\"b\\c\/d\b\f\n\r\t\u00e9\u4E2D\uD83D\uDE00z"#),
            "a\"b\\c/d\u{8}\u{C}\n\r\té中😀z"
        );
        assert_eq!(unescape("héllo".as_bytes()), "héllo");
    }
}
