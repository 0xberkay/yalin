//! Stage 1: structural scanning.
//!
//! A scanner produces the byte offsets of every *structural* position, in order:
//! - `{ } [ ] : ,` outside strings,
//! - the opening **and** closing `"` of every string (always as a pair),
//! - the first byte of every bare atom (number / literal / garbage) that follows
//!   whitespace, a structural byte or a closing quote.
//!
//! Scanning is incremental: the parser pulls offsets in chunks of about
//! [`CHUNK`] into one reused buffer, so the full index is never materialized
//! (a 1 MB document has ~370k structurals; allocating and growing that index
//! cost ~30% of parse time on device).
//!
//! The scanner only tracks quote/escape state; all grammar, UTF-8, escape and
//! number validation happens in stage 2 ([`crate::parser`]).
//!
//! [`BlockScanner`] is the production scanner (64-byte blocks, NEON on aarch64).
//! The byte-at-a-time `ScalarScanner` is kept (test builds only) as the reference
//! it is checked against.

mod block;
#[cfg(test)]
mod scalar;

pub(crate) use block::BlockScanner;
#[cfg(test)]
pub(crate) use scalar::ScalarScanner;

use crate::JsonError;

/// A [`Scanner::fill`] call stops once it has produced at least this many offsets.
pub(crate) const CHUNK: usize = 4096;

/// Output buffer length [`Scanner::fill`] needs for `input_len` bytes: one call
/// may overshoot [`CHUNK`] by up to one 64-byte block. Small inputs get a small
/// buffer (it is zeroed on allocation).
pub(crate) fn chunk_capacity(input_len: usize) -> usize {
    input_len.min(CHUNK) + 64
}

pub(crate) trait Scanner {
    /// Writes the next structural offsets (see module docs) to the start of `out`,
    /// which holds at least [`chunk_capacity`]`(input.len())` slots,
    /// and returns how many: at least [`CHUNK`] unless the input ends first, and
    /// 0 once the whole input has been scanned.
    ///
    /// Callers pass the same `input` every time and guarantee `input.len() <= u32::MAX`.
    fn fill(&mut self, input: &[u8], out: &mut [u32]) -> Result<usize, JsonError>;
}

/// Runs `S` over the whole input, concatenating every chunk.
#[cfg(test)]
pub(crate) fn drain<S: Scanner + Default>(input: &[u8]) -> Result<Vec<u32>, JsonError> {
    let (mut scanner, mut buf, mut all) = (S::default(), vec![0; chunk_capacity(input.len())], Vec::new());
    loop {
        match scanner.fill(input, &mut buf)? {
            0 => return Ok(all),
            n => all.extend_from_slice(&buf[..n]),
        }
    }
}
