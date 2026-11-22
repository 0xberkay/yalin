//! RFC 8259 number grammar: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`

use crate::{ErrorKind, JsonError};

/// Validates the number starting at `input[at]`; returns the offset one past its end.
/// The caller checks what follows (a digit after a leading `0`, for instance).
pub(crate) fn validate(input: &[u8], at: usize) -> Result<usize, JsonError> {
    // One or more digits at `i`; returns the offset after them.
    let digits = |i: usize| -> Result<usize, JsonError> {
        match digit_run(input, i) {
            0 => Err(JsonError::new(ErrorKind::InvalidNumber, i)),
            n => Ok(i + n),
        }
    };

    let mut i = at;
    if input.get(i) == Some(&b'-') {
        i += 1;
    }
    i = if input.get(i) == Some(&b'0') { i + 1 } else { digits(i)? };
    if input.get(i) == Some(&b'.') {
        i = digits(i + 1)?;
    }
    if matches!(input.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(input.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        i = digits(i)?;
    }
    Ok(i)
}

/// Number of ASCII digits starting at `input[from]`, read 8 bytes at a time
/// while 8 bytes remain.
#[inline]
fn digit_run(input: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = input.get(i..i + 8) {
        let n = leading_digits(u64::from_le_bytes(chunk.try_into().unwrap()));
        i += n;
        if n < 8 {
            return i - from;
        }
    }
    while input.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    i - from
}

/// How many of the 8 bytes of `x` (little-endian: byte 0 first) are ASCII digits
/// before the first byte that is not.
#[inline]
fn leading_digits(x: u64) -> usize {
    const HIGH_NIBBLES: u64 = u64::from_le_bytes([0xF0; 8]);
    const ZEROS: u64 = u64::from_le_bytes([b'0'; 8]);
    const SIXES: u64 = u64::from_le_bytes([0x06; 8]);
    // A byte is a digit iff its high nibble is 3 (0x30..=0x3F) and adding 6 keeps it
    // there (rules out 0x3A..=0x3F). The addition only carries out of a byte >= 0xFA,
    // which is already a non-digit, so the lowest non-digit byte is always exact.
    let bad = ((x & HIGH_NIBBLES) ^ ZEROS) | ((x.wrapping_add(SIXES) & HIGH_NIBBLES) ^ ZEROS);
    (bad.trailing_zeros() / 8) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        for ok in ["0", "-0", "7", "120", "1.5", "-0.25", "1e9", "1E+2", "2e-03", "1.0e10"] {
            assert_eq!(validate(ok.as_bytes(), 0), Ok(ok.len()), "{ok}");
        }
        // Leading zero stops after `0`; the parser rejects the following digit.
        assert_eq!(validate(b"01", 0), Ok(1));
        for bad in ["-", "-a", ".5", "1.", "1.e2", "1e", "1e+", "+1"] {
            assert!(validate(bad.as_bytes(), 0).is_err(), "{bad}");
        }
    }

    #[test]
    fn leading_digits_matches_bytewise() {
        let scalar = |b: &[u8; 8]| b.iter().take_while(|b| b.is_ascii_digit()).count();
        // Every byte value at every position, between digits.
        for pos in 0..8 {
            for byte in 0..=255u8 {
                let mut b = *b"01234567";
                b[pos] = byte;
                assert_eq!(leading_digits(u64::from_le_bytes(b)), scalar(&b), "{b:02X?}");
            }
        }
        // Mixed blocks, including carries out of 0xFA..=0xFF.
        let alphabet = b"0123456789/:.eE+-\xFA\xFF\x00 ,]";
        let mut seed = 0x9E3779B97F4A7C15u64;
        for _ in 0..50_000 {
            let b: [u8; 8] = std::array::from_fn(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                alphabet[(seed % alphabet.len() as u64) as usize]
            });
            assert_eq!(leading_digits(u64::from_le_bytes(b)), scalar(&b), "{b:02X?}");
        }
    }

    #[test]
    fn digit_runs_of_every_length() {
        for len in 0..40 {
            for tail in [&b""[..], b",", b"x", b"9"] {
                let mut s = b"-".to_vec();
                s.extend(std::iter::repeat_n(b'7', len));
                s.extend_from_slice(tail);
                let expected = len + (tail == b"9") as usize;
                assert_eq!(digit_run(&s, 1), expected, "{:?}", String::from_utf8_lossy(&s));
            }
        }
    }
}
