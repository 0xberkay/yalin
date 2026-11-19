use super::{CHUNK, Scanner};
use crate::{ErrorKind, JsonError};

/// Byte-at-a-time reference scanner. Resumable: state between [`Scanner::fill`]
/// calls is the next byte to look at and whether it continues a bare atom.
#[derive(Default)]
pub(crate) struct ScalarScanner {
    pos: usize,
    /// True while inside a bare atom, so only its first byte is recorded.
    in_atom: bool,
}

impl Scanner for ScalarScanner {
    fn fill(&mut self, input: &[u8], out: &mut [u32]) -> Result<usize, JsonError> {
        debug_assert!(input.len() <= u32::MAX as usize);
        let n = input.len();
        let mut len = 0;
        // Each step records at most two offsets, so `len` stays below CHUNK + 2.
        macro_rules! push {
            ($at:expr) => {{
                out[len] = $at as u32;
                len += 1;
            }};
        }
        let (mut i, mut in_atom) = (self.pos, self.in_atom);
        while i < n && len < CHUNK {
            match input[i] {
                b'"' => {
                    let open = i;
                    push!(open);
                    i += 1;
                    while i < n {
                        match input[i] {
                            b'\\' => i += 1,
                            b'"' => break,
                            0x00..0x20 => {
                                return Err(JsonError::new(ErrorKind::ControlCharacterInString, i));
                            }
                            _ => {}
                        }
                        // The byte after a backslash is still string content: check it too.
                        if i < n && input[i] < 0x20 {
                            return Err(JsonError::new(ErrorKind::ControlCharacterInString, i));
                        }
                        i += 1;
                    }
                    if i >= n {
                        return Err(JsonError::new(ErrorKind::UnterminatedString, open));
                    }
                    push!(i);
                    in_atom = false;
                }
                b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                    push!(i);
                    in_atom = false;
                }
                b' ' | b'\t' | b'\n' | b'\r' => in_atom = false,
                _ => {
                    if !in_atom {
                        push!(i);
                        in_atom = true;
                    }
                }
            }
            i += 1;
        }
        (self.pos, self.in_atom) = (i, in_atom);
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(s: &[u8]) -> Result<Vec<u32>, JsonError> {
        crate::scanner::drain::<ScalarScanner>(s)
    }

    #[test]
    fn records_structurals_quotes_and_atom_starts() {
        assert_eq!(
            scan(br#"{"a\"}":[12, true]}"#).unwrap(),
            vec![0, 1, 6, 7, 8, 9, 11, 13, 17, 18]
        );
    }

    #[test]
    fn escaped_backslash_before_quote_closes_string() {
        assert_eq!(scan(br#""a\\" 1"#).unwrap(), vec![0, 4, 6]);
    }

    #[test]
    fn unterminated_string_reports_opening_quote() {
        let e = scan(br#"[1, "abc\"]"#).unwrap_err();
        assert_eq!((e.kind(), e.offset()), (ErrorKind::UnterminatedString, 4));
        assert_eq!(scan(b"\"\\").unwrap_err().kind(), ErrorKind::UnterminatedString);
    }

    #[test]
    fn chunks_resume_mid_atom_and_mid_input() {
        // Long runs of atoms and strings cross many chunk boundaries; resuming must
        // neither drop nor duplicate offsets, including inside a multi-byte atom.
        let mut json = String::from("[");
        for i in 0..3 * CHUNK {
            json += &format!("{i},\"s{i}\",true,");
        }
        json += "null]";
        let offsets = scan(json.as_bytes()).unwrap();
        let mut expected = vec![0u32];
        let b = json.as_bytes();
        let (mut i, mut in_atom, mut in_str) = (1, false, false);
        while i < b.len() {
            match (in_str, b[i]) {
                (false, b'"') => {
                    expected.push(i as u32);
                    (in_str, in_atom) = (true, false);
                }
                (true, b'"') => {
                    expected.push(i as u32);
                    in_str = false;
                }
                (true, _) => {}
                (false, b',' | b']') => {
                    expected.push(i as u32);
                    in_atom = false;
                }
                (false, _) => {
                    if !in_atom {
                        expected.push(i as u32);
                        in_atom = true;
                    }
                }
            }
            i += 1;
        }
        assert_eq!(offsets, expected);
    }
}
