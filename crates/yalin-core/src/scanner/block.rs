//! Production stage 1: 64 bytes at a time with bitmasks (the simdjson approach).
//!
//! Each block is classified into one bit per byte (quote, backslash, whitespace,
//! operator) with NEON on aarch64, SSE2 on x86_64 and a portable loop elsewhere; everything after
//! that is plain `u64` arithmetic shared by all targets, so output is identical
//! everywhere:
//! - escaped bytes: walk the (rare) backslash bits, each one escapes the next byte;
//! - string region: prefix XOR of the unescaped quotes; a control byte (< 0x20)
//!   inside it is an error, so stage 2 never looks for them;
//! - atom starts: "other" bytes whose predecessor is not "other".
//!
//! Unlike [`super::ScalarScanner`], a backslash escapes the next byte even outside
//! strings. That only changes the output for input that is already invalid (a bare
//! `\` is never valid outside a string), and stage 2 still rejects it.

use super::{CHUNK, Scanner};
use crate::{ErrorKind, JsonError};

/// One bit per byte of a 64-byte block (bit `i` = byte `i`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Classes {
    quote: u64,
    backslash: u64,
    whitespace: u64,
    /// `{ } [ ] : ,`
    op: u64,
    /// Bytes below 0x20 (raw control characters; whitespace included).
    control: u64,
}

#[derive(Default)]
pub(crate) struct BlockScanner {
    /// Start of the next block.
    pos: usize,
    /// The first byte of the next block is escaped.
    escaped_carry: bool,
    /// The next block starts inside a string.
    in_string: bool,
    /// The last byte of the previous block was part of a bare atom.
    prev_other: bool,
    /// Offset of the most recent unescaped quote (the opening one, if a string is open).
    last_quote: usize,
}

impl Scanner for BlockScanner {
    fn fill(&mut self, input: &[u8], out: &mut [u32]) -> Result<usize, JsonError> {
        debug_assert!(input.len() <= u32::MAX as usize);
        let n = input.len();
        let mut len = 0;
        while self.pos < n && len < CHUNK {
            let base = self.pos;
            let classes = match input.get(base..base + 64) {
                Some(block) => classify(block.try_into().unwrap()),
                None => {
                    // Pad the tail with spaces: whitespace adds no structurals.
                    let mut block = [b' '; 64];
                    block[..n - base].copy_from_slice(&input[base..]);
                    classify(&block)
                }
            };
            let bits = self.structurals(classes, base)?;
            len += write_offsets(&mut out[len..len + 64], base as u32, bits);
            self.pos = base + 64;
        }
        if self.pos >= n && self.in_string {
            return Err(JsonError::new(ErrorKind::UnterminatedString, self.last_quote));
        }
        Ok(len)
    }
}

impl BlockScanner {
    fn structurals(&mut self, c: Classes, base: usize) -> Result<u64, JsonError> {
        let quote = c.quote & !self.escaped(c.backslash);
        // Bit set from an opening quote (inclusive) to its closing quote (exclusive).
        let in_string = prefix_xor(quote) ^ if self.in_string { !0 } else { 0 };
        // Quotes are not control bytes, so this is exactly the string contents.
        let control = c.control & in_string;
        if control != 0 {
            let at = base + control.trailing_zeros() as usize;
            return Err(JsonError::new(ErrorKind::ControlCharacterInString, at));
        }
        self.in_string = in_string >> 63 == 1;
        if quote != 0 {
            self.last_quote = base + 63 - quote.leading_zeros() as usize;
        }
        let string = in_string | quote;
        let other = !(c.whitespace | c.op | string);
        let atom_start = other & !(other << 1 | self.prev_other as u64);
        self.prev_other = other >> 63 == 1;
        Ok((c.op & !string) | quote | atom_start)
    }

    /// Bits of bytes escaped by a preceding (itself unescaped) backslash.
    fn escaped(&mut self, backslash: u64) -> u64 {
        let mut escaped = self.escaped_carry as u64;
        self.escaped_carry = false;
        let mut pending = backslash & !escaped;
        while pending != 0 {
            let i = pending.trailing_zeros();
            if i == 63 {
                self.escaped_carry = true;
            } else {
                escaped |= 1 << (i + 1);
            }
            // The escaped byte cannot start an escape of its own.
            pending &= !(3u64 << i);
        }
        escaped
    }
}

/// Writes `base + i` for every set bit `i` of `bits` to the start of `dst` and
/// returns how many. (simdjson's unconditional 8-wide writes measured no faster
/// on the A76 here.)
#[inline]
fn write_offsets(dst: &mut [u32], base: u32, mut bits: u64) -> usize {
    let mut count = 0;
    while bits != 0 {
        dst[count] = base.wrapping_add(bits.trailing_zeros());
        bits &= bits - 1;
        count += 1;
    }
    count
}

/// Bit `i` of the result is the XOR of bits `0..=i` of `x`.
#[inline]
fn prefix_xor(mut x: u64) -> u64 {
    x ^= x << 1;
    x ^= x << 2;
    x ^= x << 4;
    x ^= x << 8;
    x ^= x << 16;
    x ^= x << 32;
    x
}

#[cfg(any(test, not(any(target_arch = "aarch64", target_arch = "x86_64"))))]
fn classify_portable(block: &[u8; 64]) -> Classes {
    let mut c = Classes::default();
    for (i, &b) in block.iter().enumerate() {
        let bit = 1u64 << i;
        match b {
            b'"' => c.quote |= bit,
            b'\\' => c.backslash |= bit,
            b' ' => c.whitespace |= bit,
            b'\t' | b'\n' | b'\r' => {
                c.whitespace |= bit;
                c.control |= bit;
            }
            b'{' | b'}' | b'[' | b']' | b':' | b',' => c.op |= bit,
            0x00..0x20 => c.control |= bit,
            _ => {}
        }
    }
    c
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
use classify_portable as classify;

#[cfg(target_arch = "x86_64")]
use sse2::classify;

#[cfg(target_arch = "x86_64")]
mod sse2 {
    use super::Classes;
    use core::arch::x86_64::*;

    pub(super) fn classify(block: &[u8; 64]) -> Classes {
        // SAFETY: SSE2 is part of the x86_64 baseline, and every unaligned load
        // reads 16 bytes inside `block` (offsets 0, 16, 32, 48 of a 64-byte array).
        unsafe {
            let p = block.as_ptr().cast::<__m128i>();
            let v = [0, 1, 2, 3].map(|i| _mm_loadu_si128(p.add(i)));
            let mask = |f: &dyn Fn(__m128i) -> __m128i| {
                v.iter().enumerate().fold(0u64, |acc, (i, &x)| {
                    acc | (_mm_movemask_epi8(f(x)) as u16 as u64) << (16 * i)
                })
            };
            let eq = |x, c: u8| _mm_cmpeq_epi8(x, _mm_set1_epi8(c as i8));
            Classes {
                quote: mask(&|x| eq(x, b'"')),
                backslash: mask(&|x| eq(x, b'\\')),
                whitespace: mask(&|x| {
                    _mm_or_si128(_mm_or_si128(eq(x, b' '), eq(x, b'\t')), _mm_or_si128(eq(x, b'\n'), eq(x, b'\r')))
                }),
                op: mask(&|x| {
                    // '[' | 0x20 == '{' and ']' | 0x20 == '}'; no other byte maps there.
                    let f = _mm_or_si128(x, _mm_set1_epi8(0x20));
                    _mm_or_si128(_mm_or_si128(eq(f, b'{'), eq(f, b'}')), _mm_or_si128(eq(x, b':'), eq(x, b',')))
                }),
                // Unsigned x <= 0x1F: min(x, 0x1F) == x.
                control: mask(&|x| _mm_cmpeq_epi8(_mm_min_epu8(x, _mm_set1_epi8(0x1F)), x)),
            }
        }
    }
}

#[cfg(target_arch = "aarch64")]
use neon::classify;

#[cfg(target_arch = "aarch64")]
mod neon {
    use super::Classes;
    use core::arch::aarch64::*;

    pub(super) fn classify(block: &[u8; 64]) -> Classes {
        // SAFETY: NEON is part of the aarch64 baseline, and every load reads
        // 16 bytes inside `block` (offsets 0, 16, 32, 48 of a 64-byte array).
        unsafe {
            let p = block.as_ptr();
            let v = [vld1q_u8(p), vld1q_u8(p.add(16)), vld1q_u8(p.add(32)), vld1q_u8(p.add(48))];
            let eq = |c: u8| v.map(|x| vceqq_u8(x, vdupq_n_u8(c)));
            let or = |a: [uint8x16_t; 4], b: [uint8x16_t; 4]| [0, 1, 2, 3].map(|i| vorrq_u8(a[i], b[i]));
            // '[' | 0x20 == '{' and ']' | 0x20 == '}'; no other byte maps there.
            let folded = v.map(|x| vorrq_u8(x, vdupq_n_u8(0x20)));
            let fold_eq = |c: u8| folded.map(|x| vceqq_u8(x, vdupq_n_u8(c)));
            Classes {
                quote: movemask(eq(b'"')),
                backslash: movemask(eq(b'\\')),
                whitespace: movemask(or(or(eq(b' '), eq(b'\t')), or(eq(b'\n'), eq(b'\r')))),
                op: movemask(or(or(fold_eq(b'{'), fold_eq(b'}')), or(eq(b':'), eq(b',')))),
                control: movemask(v.map(|x| vcltq_u8(x, vdupq_n_u8(0x20)))),
            }
        }
    }

    /// 64 comparison lanes (0x00 / 0xFF) to a bitmask, lane `i` -> bit `i`.
    #[inline]
    unsafe fn movemask(m: [uint8x16_t; 4]) -> u64 {
        const WEIGHTS: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
        // SAFETY: plain register arithmetic; the load reads the 16-byte const array.
        unsafe {
            let w = vld1q_u8(WEIGHTS.as_ptr());
            let t = m.map(|x| vandq_u8(x, w));
            let s0 = vpaddq_u8(t[0], t[1]);
            let s1 = vpaddq_u8(t[2], t[3]);
            let s = vpaddq_u8(s0, s1);
            let s = vpaddq_u8(s, s);
            vgetq_lane_u64(vreinterpretq_u64_u8(s), 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ScalarScanner, drain};

    /// Whether a backslash occurs outside a string, the one case where the
    /// scanners may legitimately differ (see module docs).
    fn backslash_outside_string(s: &[u8]) -> bool {
        let (mut i, mut in_string) = (0, false);
        while i < s.len() {
            match (in_string, s[i]) {
                (true, b'\\') => i += 1,
                (_, b'"') => in_string = !in_string,
                (false, b'\\') => return true,
                _ => {}
            }
            i += 1;
        }
        false
    }

    /// Asserts both scanners agree exactly; returns false (skipping) where they may not.
    fn assert_same(input: &[u8]) -> bool {
        if backslash_outside_string(input) {
            // Both must still reject the document.
            assert!(crate::validate(input).is_err());
            return false;
        }
        assert_eq!(
            drain::<BlockScanner>(input),
            drain::<ScalarScanner>(input),
            "{:?}",
            String::from_utf8_lossy(input)
        );
        true
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }
    }

    #[test]
    fn hand_picked() {
        for s in [
            &br#"{"a\"}":[12, true]}"#[..],
            br#""a\\" 1"#,
            br#"[1, "abc\"]"#,
            b"\"\\",
            b"",
            b"   ",
            b"tru e",
            b"12\"x\"34",
            b"\"\"\"\"",
        ] {
            assert!(assert_same(s));
        }
    }

    /// Escapes, quotes and atoms placed on and around every block boundary.
    #[test]
    fn block_boundaries() {
        let mut compared = 0;
        for pad in 0..130 {
            for tail in [&br#""\\\"x""#[..], br#""\\\\"1"#, br#""a"b"#, b"tru,e", b"\"x", br#""\"#] {
                let mut s = vec![b' '; pad];
                s.extend_from_slice(tail);
                s.extend_from_slice(b" [1]");
                compared += assert_same(&s) as usize;
                let mut s = vec![b'"'];
                s.extend(std::iter::repeat_n(b'\\', pad));
                s.extend_from_slice(tail);
                compared += assert_same(&s) as usize;
            }
        }
        assert!(compared > 1000, "{compared}");
    }

    /// Random token soups (backslashes only inside strings, so both scanners must
    /// agree exactly, errors included), long enough to cross many blocks and chunks.
    #[test]
    fn random_matches_scalar() {
        let mut rng = Rng(0x2545F4914F6CDD1D);
        let mut compared = 0;
        for round in 0..3000 {
            let mut s = Vec::new();
            let tokens = if round % 100 == 0 { 3000 } else { rng.next(60) };
            for t in 0..tokens {
                match rng.next(9) {
                    0 => s.push(b"{}[]:,"[rng.next(6)]),
                    1 => s.push(b" \t\n\r"[rng.next(4)]),
                    2 => s.extend_from_slice([&b"1"[..], b"-2.5e3", b"true", b"null", b"x", b"\xC3\xBC"][rng.next(6)]),
                    3 => s.extend(std::iter::repeat_n(b' ', rng.next(70))),
                    _ => {
                        s.push(b'"');
                        for _ in 0..rng.next(80) {
                            match rng.next(8) {
                                // Complete escapes only, so the string stays open until its quote.
                                0 => (0..1 + rng.next(4)).for_each(|_| s.extend_from_slice(br"\\")),
                                1 => s.extend_from_slice(br#"\""#),
                                2 => s.extend_from_slice(b"{,: ]"),
                                3 => s.extend_from_slice(br"\n"),
                                4 if rng.next(300) == 0 => s.push(b"\x01\t\n\x1F"[rng.next(4)]),
                                _ => s.push(b"az09 \xC3\xBCq"[rng.next(8)]),
                            }
                        }
                        // Sometimes leave the last string unterminated.
                        if t + 1 < tokens || rng.next(5) != 0 {
                            s.push(b'"');
                        }
                    }
                }
            }
            compared += assert_same(&s) as usize;
        }
        assert_eq!(compared, 3000);
    }

    #[test]
    fn classify_matches_portable() {
        let mut rng = Rng(0x9E3779B97F4A7C15);
        let alphabet = b" \t\n\r\"\\{}[]:,azAZ09[{]}\x00\x20\x5b\x5d\x7b\x7d\x80\xFF\x0b\x0c";
        for _ in 0..20_000 {
            let block: [u8; 64] = std::array::from_fn(|_| alphabet[rng.next(alphabet.len())]);
            assert_eq!(classify(&block), classify_portable(&block));
        }
        let all: Vec<u8> = (0..=255).collect();
        for chunk in all.chunks(64) {
            let block: [u8; 64] = chunk.try_into().unwrap();
            assert_eq!(classify(&block), classify_portable(&block));
        }
    }
}
