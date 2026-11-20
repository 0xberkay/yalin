//! Whole-tree wire format, decoded by `YalinTreeDecoder.kt`. Keep both in sync.
//! All integers are little-endian.
//!
//! `[i64 status]` then, if `status == 0`, `[u8 slot_bits]` and the root value
//! (`slot_bits` is explained below). A non-zero status is
//! a parse error code ([`codec::parse_error_code`]); a repeated object key is
//! reported as one with kind `DuplicateKey`.
//!
//! A value is `[u8 tag][payload]` (tags shared with [`codec`] where they overlap):
//!
//! | tag | payload |
//! |-----|---------|
//! | 1 `NULL`, 2 `FALSE`, 3 `TRUE` | – |
//! | 4 `INT` | `i64` |
//! | 5 `DOUBLE` | `f64` (has a fraction or exponent, finite) |
//! | 6 `BIG_NUMBER` | `[u32 len][ASCII]` (integer outside `i64`, or overflows `f64`) |
//! | 7 `STRING` | `[u32 len][UTF-8]` |
//! | 8 `ARRAY` | `[u32 count]` then `count` values |
//! | 9 `OBJECT` | `[u32 count]` then `count` × (key, value) |
//! | 12 `STRING_NEW` | `[u16 slot][u8 len][UTF-8]`: also store it in string slot `slot` |
//! | 13 `STRING_REF` | `[u16 slot]`: the string last stored in that slot |
//!
//! A key is `[u8 kind]` then `KEY` (0) `[u32 len][UTF-8]`, `KEY_NEW` (1)
//! `[u16 slot][u8 len][UTF-8]` or `KEY_REF` (2) `[u16 slot]`, with key slots
//! separate from string slots.
//!
//! Slots let the decoder build each repeated key or short string once. They
//! form a direct-mapped cache of `1 << slot_bits` entries chosen by hash, sized
//! to the input so small inputs stay cheap. The encoder decides every store and
//! hit, so the decoder only mirrors the slot arrays.

use yalin_core::{Document, Visitor};

use crate::codec::{self, ARRAY, BIG_NUMBER, DOUBLE, FALSE, INT, NULL, OBJECT, STRING, TRUE, put_bytes};

pub const STRING_NEW: u8 = 12;
pub const STRING_REF: u8 = 13;
pub const KEY: u8 = 0;
pub const KEY_NEW: u8 = 1;
pub const KEY_REF: u8 = 2;

/// Slot count bounds (per cache), as powers of two.
const MIN_SLOT_BITS: u32 = 6;
const MAX_SLOT_BITS: u32 = 10;
/// Longer strings are never cached: they rarely repeat, and hashing them costs.
const MAX_CACHED_LEN: usize = 32;

/// Parses `input` and encodes its whole tree, or the parse/duplicate-key failure.
pub fn encode(input: &[u8]) -> Vec<u8> {
    let doc = match Document::parse(input) {
        Ok(doc) => doc,
        Err(e) => return codec::parse_failure(&e),
    };
    // Encoded size is close to the input size: about as many tag and length
    // bytes are added as quotes and punctuation are dropped.
    let mut out = Vec::with_capacity(9 + input.len() + input.len() / 4);
    out.extend_from_slice(&0i64.to_le_bytes());
    let bits = slot_bits(input.len());
    out.push(bits as u8);
    let mut encoder = Encoder { out, counts: Vec::with_capacity(16), keys: Cache::new(bits), strings: Cache::new(bits) };
    match doc.root().walk(&mut encoder) {
        Ok(()) => encoder.out,
        Err(e) => codec::parse_failure(&e),
    }
}

/// About one slot per 64 input bytes, within the bounds.
fn slot_bits(input_len: usize) -> u32 {
    (input_len / 64).max(1).ilog2().clamp(MIN_SLOT_BITS, MAX_SLOT_BITS)
}

/// Direct-mapped cache of strings already written to the output.
struct Cache {
    bits: u32,
    /// Output span `(offset, len)` of the string stored in each slot; `len == usize::MAX` when empty.
    slots: Box<[(usize, usize)]>,
}

enum Lookup {
    Hit(u16),
    Store(u16),
    Uncached,
}

impl Cache {
    fn new(bits: u32) -> Self {
        Self { bits, slots: vec![(0, usize::MAX); 1 << bits].into_boxed_slice() }
    }

    fn lookup(&mut self, out: &[u8], s: &[u8]) -> Lookup {
        if s.len() > MAX_CACHED_LEN {
            return Lookup::Uncached;
        }
        let slot = slot(s, self.bits);
        let (at, len) = self.slots[slot];
        if len == s.len() && &out[at..at + len] == s {
            return Lookup::Hit(slot as u16);
        }
        // The bytes go after the 1-byte tag, 2-byte slot and 1-byte length.
        self.slots[slot] = (out.len() + 4, s.len());
        Lookup::Store(slot as u16)
    }
}

/// Cache slot for a short string: a multiplicative hash of its first and last
/// bytes (up to 8 each) and its length. Collisions only cost a re-store.
fn slot(s: &[u8], bits: u32) -> usize {
    let n = s.len();
    let word = |at: usize| u64::from_le_bytes(s[at..at + 8].try_into().unwrap_or_default());
    let half = |at: usize| u32::from_le_bytes(s[at..at + 4].try_into().unwrap_or_default()) as u64;
    let w = match n {
        8.. => word(0) ^ word(n - 8).rotate_left(29),
        4.. => half(0) | half(n - 4) << 32,
        1.. => s[0] as u64 | (s[n / 2] as u64) << 8 | (s[n - 1] as u64) << 16,
        0 => 0,
    };
    ((w ^ n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - bits)) as usize
}

struct Encoder {
    out: Vec<u8>,
    /// Output offsets of the counts of open containers, patched when they end.
    counts: Vec<usize>,
    keys: Cache,
    strings: Cache,
}

impl Encoder {
    fn begin(&mut self, tag: u8) {
        self.out.push(tag);
        self.counts.push(self.out.len());
        self.out.extend_from_slice(&[0; 4]);
    }

    fn end(&mut self, len: usize) {
        let at = self.counts.pop().unwrap_or_default();
        // Fits: a container has fewer children than the input (< u32::MAX) has bytes.
        self.out[at..at + 4].copy_from_slice(&(len as u32).to_le_bytes());
    }

    /// Writes `s` as a reference, a stored or an uncached string, with the given tags.
    fn cached(&mut self, key: bool, s: &str, [plain, new, reference]: [u8; 3]) {
        let s = s.as_bytes();
        let cache = if key { &mut self.keys } else { &mut self.strings };
        match cache.lookup(&self.out, s) {
            Lookup::Hit(slot) => {
                self.out.push(reference);
                self.out.extend_from_slice(&slot.to_le_bytes());
            }
            Lookup::Store(slot) => {
                self.out.push(new);
                self.out.extend_from_slice(&slot.to_le_bytes());
                self.out.push(s.len() as u8);
                self.out.extend_from_slice(s);
            }
            Lookup::Uncached => {
                self.out.push(plain);
                put_bytes(&mut self.out, s);
            }
        }
    }
}

impl Visitor for Encoder {
    fn null(&mut self) {
        self.out.push(NULL);
    }

    fn bool(&mut self, value: bool) {
        self.out.push(if value { TRUE } else { FALSE });
    }

    fn number(&mut self, text: &str) {
        let integer = !text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'));
        if integer && let Ok(i) = text.parse::<i64>() {
            self.out.push(INT);
            self.out.extend_from_slice(&i.to_le_bytes());
            return;
        }
        match text.parse::<f64>() {
            Ok(f) if !integer && f.is_finite() => {
                self.out.push(DOUBLE);
                self.out.extend_from_slice(&f.to_le_bytes());
            }
            _ => {
                self.out.push(BIG_NUMBER);
                put_bytes(&mut self.out, text.as_bytes());
            }
        }
    }

    fn string(&mut self, value: &str) {
        self.cached(false, value, [STRING, STRING_NEW, STRING_REF]);
    }

    fn key(&mut self, key: &str) {
        self.cached(true, key, [KEY, KEY_NEW, KEY_REF]);
    }

    fn begin_array(&mut self) {
        self.begin(ARRAY);
    }

    fn end_array(&mut self, len: usize) {
        self.end(len);
    }

    fn begin_object(&mut self) {
        self.begin(OBJECT);
    }

    fn end_object(&mut self, len: usize) {
        self.end(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slot of `s` in a small input's caches.
    fn le16(s: &[u8]) -> [u8; 2] {
        (slot(s, MIN_SLOT_BITS) as u16).to_le_bytes()
    }

    fn header() -> Vec<u8> {
        let mut h = 0i64.to_le_bytes().to_vec();
        h.push(MIN_SLOT_BITS as u8);
        h
    }

    #[test]
    fn encodes_every_value() {
        let long = "x".repeat(33);
        let input = format!(
            r#"{{"a":[1,-2.5,1e2,123456789012345678901,1e400],"é":{{"n":null,"t":true,"f":false}},"s":"x\ty","{long}":"{long}"}}"#
        );
        let out = encode(input.as_bytes());
        let mut want = header();
        want.extend([OBJECT, 4, 0, 0, 0]);
        let key = |want: &mut Vec<u8>, k: &str| {
            want.push(KEY_NEW);
            want.extend(le16(k.as_bytes()));
            want.push(k.len() as u8);
            want.extend(k.as_bytes());
        };
        key(&mut want, "a");
        want.extend([ARRAY, 5, 0, 0, 0, INT]);
        want.extend(1i64.to_le_bytes());
        want.push(DOUBLE);
        want.extend((-2.5f64).to_le_bytes());
        want.push(DOUBLE);
        want.extend(100f64.to_le_bytes());
        want.extend([BIG_NUMBER, 21, 0, 0, 0]);
        want.extend(b"123456789012345678901");
        want.extend([BIG_NUMBER, 5, 0, 0, 0]);
        want.extend(b"1e400");
        key(&mut want, "é");
        want.extend([OBJECT, 3, 0, 0, 0]);
        key(&mut want, "n");
        want.push(NULL);
        key(&mut want, "t");
        want.push(TRUE);
        key(&mut want, "f");
        want.push(FALSE);
        key(&mut want, "s");
        want.push(STRING_NEW);
        want.extend(le16(b"x\ty"));
        want.extend([3, b'x', b'\t', b'y']);
        want.extend([KEY, 33, 0, 0, 0]);
        want.extend(long.as_bytes());
        want.extend([STRING, 33, 0, 0, 0]);
        want.extend(long.as_bytes());
        assert_eq!(out, want);
    }

    #[test]
    fn repeats_become_references() {
        let out = encode(br#"[{"k":"v"},{"k":"v"},"k"]"#);
        let (k, v) = (le16(b"k"), le16(b"v"));
        let mut want = header();
        want.extend([ARRAY, 3, 0, 0, 0, OBJECT, 1, 0, 0, 0, KEY_NEW, k[0], k[1], 1, b'k']);
        want.extend([STRING_NEW, v[0], v[1], 1, b'v']);
        want.extend([OBJECT, 1, 0, 0, 0, KEY_REF, k[0], k[1], STRING_REF, v[0], v[1]]);
        // Keys and strings have separate slots.
        want.extend([STRING_NEW, k[0], k[1], 1, b'k']);
        assert_eq!(out, want);
    }

    #[test]
    fn colliding_strings_replace_each_other() {
        let mut by_slot = std::collections::HashMap::new();
        let (a, b) = (0..)
            .map(|i| format!("s{i}"))
            .find_map(|s| by_slot.insert(slot(s.as_bytes(), MIN_SLOT_BITS), s.clone()).map(|prev| (prev, s)))
            .unwrap();
        let input = format!(r#"["{a}","{b}","{a}","{a}"]"#);
        let out = encode(input.as_bytes());
        let tags: Vec<u8> = {
            let mut tags = Vec::new();
            let mut i = 14;
            while i < out.len() {
                tags.push(out[i]);
                i += if out[i] == STRING_NEW { 4 + out[i + 3] as usize } else { 3 };
            }
            tags
        };
        assert_eq!(tags, [STRING_NEW, STRING_NEW, STRING_NEW, STRING_REF]);
    }

    #[test]
    fn slots_scale_with_input() {
        assert_eq!(slot_bits(0), MIN_SLOT_BITS);
        assert_eq!(slot_bits(1 << 10), MIN_SLOT_BITS);
        assert_eq!(slot_bits(16 << 10), 8);
        assert_eq!(slot_bits(1 << 20), MAX_SLOT_BITS);
        for s in ["", "a", "abc", "abcd", "abcdefg", "abcdefgh", "SKU-000123", &"x".repeat(32)] {
            assert!(slot(s.as_bytes(), MIN_SLOT_BITS) < 1 << MIN_SLOT_BITS);
            assert!(slot(s.as_bytes(), MAX_SLOT_BITS) < 1 << MAX_SLOT_BITS);
        }
    }

    #[test]
    fn failures() {
        let code = |input: &[u8]| -(i64::from_le_bytes(encode(input)[..8].try_into().unwrap()) + 1);
        assert_eq!(encode(b"7"), [&header()[..], &[INT], &7i64.to_le_bytes()].concat());
        assert_eq!(code(b"[1,]") & 0xFF, 2);
        let dup = code(br#"{"a":{"b":1,"b":2}}"#);
        assert_eq!((dup & 0xFF, dup >> 8), (12, 12));
        assert_eq!(encode(br#"{"a":1,"a":2}"#).len(), 8);
    }
}
