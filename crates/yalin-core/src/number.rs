//! RFC 8259 number grammar: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`

use crate::{ErrorKind, JsonError};

/// Validates the number starting at `input[at]`; returns the offset one past its end.
/// The caller checks what follows (a digit after a leading `0`, for instance).
pub(crate) fn validate(input: &[u8], at: usize) -> Result<usize, JsonError> {
    let digit = |i: usize| input.get(i).is_some_and(u8::is_ascii_digit);
    let digits = |mut i: usize| -> Result<usize, JsonError> {
        if !digit(i) {
            return Err(JsonError::new(ErrorKind::InvalidNumber, i));
        }
        while digit(i) {
            i += 1;
        }
        Ok(i)
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
}
