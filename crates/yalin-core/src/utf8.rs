//! Whole-input UTF-8 validation (RFC 8259 §8.1: a JSON text is UTF-8).
//!
//! aarch64 uses the Keiser–Lemire "lookup" algorithm (as in simdjson): three
//! 16-entry table lookups classify each byte pair, and a continuation check
//! covers 3- and 4-byte sequences; all-ASCII 64-byte blocks are skipped. It
//! only answers "valid or not"; on failure (and on other targets) the exact
//! error offset comes from `std::str::from_utf8`, so NEON can never change an
//! error, only find valid input faster.

/// Checks that `input` is valid UTF-8; on failure returns the offset of the
/// first invalid byte (`Utf8Error::valid_up_to`).
pub(crate) fn validate(input: &[u8]) -> Result<(), usize> {
    #[cfg(target_arch = "aarch64")]
    if neon::is_valid(input) {
        return Ok(());
    }
    std::str::from_utf8(input).map(drop).map_err(|e| e.valid_up_to())
}

#[cfg(target_arch = "aarch64")]
mod neon {
    use core::arch::aarch64::*;

    // Error classes, one bit each; a byte pair is invalid if all three lookups
    // agree on some class (see `check`).
    const TOO_SHORT: u8 = 1 << 0; // lead byte or ASCII, then a lead byte or ASCII where a continuation was due
    const TOO_LONG: u8 = 1 << 1; // ASCII, then a continuation
    const OVERLONG_3: u8 = 1 << 2; // E0 followed by 80..9F
    const TOO_LARGE: u8 = 1 << 3; // above U+10FFFF
    const SURROGATE: u8 = 1 << 4; // ED followed by A0..BF
    const OVERLONG_2: u8 = 1 << 5; // C0 / C1
    const TOO_LARGE_1000: u8 = 1 << 6;
    const OVERLONG_4: u8 = 1 << 6; // F0 followed by 80..8F
    const TWO_CONTS: u8 = 1 << 7; // continuation after continuation (fixed up for 3/4-byte sequences)
    const CARRY: u8 = TOO_SHORT | TOO_LONG | TWO_CONTS;

    /// Indexed by the high nibble of the first byte of a pair.
    const BYTE_1_HIGH: [u8; 16] = [
        TOO_LONG,
        TOO_LONG,
        TOO_LONG,
        TOO_LONG,
        TOO_LONG,
        TOO_LONG,
        TOO_LONG,
        TOO_LONG,
        TWO_CONTS,
        TWO_CONTS,
        TWO_CONTS,
        TWO_CONTS,
        TOO_SHORT | OVERLONG_2,
        TOO_SHORT,
        TOO_SHORT | OVERLONG_3 | SURROGATE,
        TOO_SHORT | TOO_LARGE | TOO_LARGE_1000 | OVERLONG_4,
    ];

    /// Indexed by the low nibble of the first byte of a pair.
    const BYTE_1_LOW: [u8; 16] = [
        CARRY | OVERLONG_3 | OVERLONG_2 | OVERLONG_4,
        CARRY | OVERLONG_2,
        CARRY,
        CARRY,
        CARRY | TOO_LARGE,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000 | SURROGATE,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
        CARRY | TOO_LARGE | TOO_LARGE_1000,
    ];

    /// Indexed by the high nibble of the second byte of a pair.
    const BYTE_2_HIGH: [u8; 16] = [
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_LONG | OVERLONG_2 | TWO_CONTS | OVERLONG_3 | TOO_LARGE_1000 | OVERLONG_4,
        TOO_LONG | OVERLONG_2 | TWO_CONTS | OVERLONG_3 | TOO_LARGE,
        TOO_LONG | OVERLONG_2 | TWO_CONTS | SURROGATE | TOO_LARGE,
        TOO_LONG | OVERLONG_2 | TWO_CONTS | SURROGATE | TOO_LARGE,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
        TOO_SHORT,
    ];

    /// A vector ending in a lead byte whose sequence does not fit is incomplete:
    /// lane 13 > EF (4-byte lead), lane 14 > DF (3/4-byte lead), lane 15 > BF.
    const INCOMPLETE_MAX: [u8; 16] = [
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xEF, 0xDF, 0xBF,
    ];

    struct Validator {
        tables: [uint8x16_t; 3],
        incomplete_max: uint8x16_t,
        /// Any nonzero lane: invalid.
        error: uint8x16_t,
        /// Last 16 bytes of the previous block.
        prev: uint8x16_t,
        /// The previous block ended in an unfinished sequence.
        prev_incomplete: uint8x16_t,
    }

    pub(super) fn is_valid(input: &[u8]) -> bool {
        // SAFETY: NEON is part of the aarch64 baseline. Every load reads 16 bytes
        // inside a 16-byte const table or a 64-byte block (`chunks_exact(64)` or
        // the zero-padded tail copy).
        unsafe {
            let mut v = Validator {
                tables: [
                    vld1q_u8(BYTE_1_HIGH.as_ptr()),
                    vld1q_u8(BYTE_1_LOW.as_ptr()),
                    vld1q_u8(BYTE_2_HIGH.as_ptr()),
                ],
                incomplete_max: vld1q_u8(INCOMPLETE_MAX.as_ptr()),
                error: vdupq_n_u8(0),
                prev: vdupq_n_u8(0),
                prev_incomplete: vdupq_n_u8(0),
            };
            let mut blocks = input.chunks_exact(64);
            for block in &mut blocks {
                v.block(block.as_ptr());
            }
            let rest = blocks.remainder();
            if !rest.is_empty() {
                // Zero padding is ASCII, so an unfinished tail shows up as TOO_SHORT.
                let mut tail = [0u8; 64];
                tail[..rest.len()].copy_from_slice(rest);
                v.block(tail.as_ptr());
            }
            vmaxvq_u8(vorrq_u8(v.error, v.prev_incomplete)) == 0
        }
    }

    impl Validator {
        /// # Safety
        /// `p` must point to 64 readable bytes.
        #[inline(always)]
        unsafe fn block(&mut self, p: *const u8) {
            // SAFETY: the caller guarantees 64 readable bytes at `p`.
            unsafe {
                let b = [vld1q_u8(p), vld1q_u8(p.add(16)), vld1q_u8(p.add(32)), vld1q_u8(p.add(48))];
                let any = vorrq_u8(vorrq_u8(b[0], b[1]), vorrq_u8(b[2], b[3]));
                if vmaxvq_u8(any) < 0x80 {
                    // ASCII only: fine unless the previous block left a sequence open.
                    self.error = vorrq_u8(self.error, self.prev_incomplete);
                    self.prev_incomplete = vdupq_n_u8(0);
                } else {
                    let e = vorrq_u8(
                        vorrq_u8(self.check(b[0], self.prev), self.check(b[1], b[0])),
                        vorrq_u8(self.check(b[2], b[1]), self.check(b[3], b[2])),
                    );
                    self.error = vorrq_u8(self.error, e);
                    self.prev_incomplete = vqsubq_u8(b[3], self.incomplete_max);
                }
                self.prev = b[3];
            }
        }

        /// Error lanes for `input`, given the 16 bytes before it.
        #[inline(always)]
        unsafe fn check(&self, input: uint8x16_t, prev: uint8x16_t) -> uint8x16_t {
            // SAFETY: register-only NEON arithmetic.
            unsafe {
                let prev1 = vextq_u8(prev, input, 15);
                let byte_1_high = vqtbl1q_u8(self.tables[0], vshrq_n_u8(prev1, 4));
                let byte_1_low = vqtbl1q_u8(self.tables[1], vandq_u8(prev1, vdupq_n_u8(0x0F)));
                let byte_2_high = vqtbl1q_u8(self.tables[2], vshrq_n_u8(input, 4));
                let special = vandq_u8(vandq_u8(byte_1_high, byte_1_low), byte_2_high);
                // Third / fourth byte of a 3- / 4-byte sequence must be a continuation:
                // that turns the TWO_CONTS bit into "expected", flipped by the XOR.
                let prev2 = vextq_u8(prev, input, 14);
                let prev3 = vextq_u8(prev, input, 13);
                let is_third = vqsubq_u8(prev2, vdupq_n_u8(0xE0 - 0x80));
                let is_fourth = vqsubq_u8(prev3, vdupq_n_u8(0xF0 - 0x80));
                let must_be_cont = vandq_u8(vorrq_u8(is_third, is_fourth), vdupq_n_u8(0x80));
                veorq_u8(must_be_cont, special)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(input: &[u8]) {
        let expected = std::str::from_utf8(input).map(drop).map_err(|e| e.valid_up_to());
        #[cfg(target_arch = "aarch64")]
        assert_eq!(neon::is_valid(input), expected.is_ok(), "{input:02X?}");
        assert_eq!(validate(input), expected, "{input:02X?}");
    }

    /// Boundary-heavy byte values: ASCII, every continuation range edge, every lead-byte edge.
    const BYTES: [u8; 22] = [
        0x20, 0x41, 0x7F, 0x80, 0x8F, 0x90, 0x9F, 0xA0, 0xBF, 0xC0, 0xC1, 0xC2, 0xDF, 0xE0, 0xE1, 0xED,
        0xEE, 0xF0, 0xF1, 0xF4, 0xF5, 0xFF,
    ];

    fn sequences(max_len: usize) -> Vec<Vec<u8>> {
        let mut all: Vec<Vec<u8>> = vec![vec![]];
        let mut last = all.clone();
        for _ in 0..max_len {
            last = last.iter().flat_map(|s| BYTES.iter().map(move |&b| [s.as_slice(), &[b]].concat())).collect();
            all.extend(last.iter().cloned());
        }
        all
    }

    /// Sequences placed around 16-byte vector and 64-byte block boundaries,
    /// including ending exactly at the end of a block.
    #[test]
    fn sequences_at_boundaries() {
        let short = sequences(3);
        for seq in &short {
            for at in [0, 1, 13, 14, 15, 16, 30, 47, 60, 61, 62, 63, 64, 65, 125, 127] {
                for tail in [0, 1, 3] {
                    let mut input = vec![b'a'; at];
                    input.extend_from_slice(seq);
                    input.extend(std::iter::repeat_n(b'z', tail));
                    check(&input);
                }
            }
        }
        for seq in sequences(4).iter().filter(|s| s.len() == 4) {
            for at in [0, 12, 60, 61, 62] {
                let mut input = vec![b'a'; at];
                input.extend_from_slice(seq);
                check(&input);
                input.push(b'z');
                check(&input);
            }
        }
    }

    #[test]
    fn real_text_and_random_bytes() {
        let text = "Café 42 \"élég\" thé · 中文 😀 Vendeur ÉÀÈÔÛÇ éàèôûç €".repeat(9);
        for start in 0..text.len() {
            check(&text.as_bytes()[start..]);
        }
        let chars: Vec<char> = text.chars().collect();
        let mut x: u64 = 0x2545F4914F6CDD1D;
        for _ in 0..200_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let len = (x % 150) as usize;
            let mut input = Vec::with_capacity(len);
            let mut y = x;
            for _ in 0..len {
                y ^= y << 13;
                y ^= y >> 7;
                y ^= y << 17;
                // Mostly valid characters with occasional raw edge bytes.
                if y % 11 == 0 {
                    input.push(BYTES[(y >> 8) as usize % BYTES.len()]);
                } else {
                    let c = chars[(y >> 8) as usize % chars.len()];
                    input.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
            }
            check(&input);
        }
    }
}
