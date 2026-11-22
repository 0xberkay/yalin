//! C entry points for the Swift package (`apple/Sources/Yalin`). The matching
//! declarations are in `apple/Sources/YalinFFI/yalin.h`; keep both in sync.
//!
//! Ownership rules:
//! - Input pointers are only read during the call. One-shot calls
//!   ([`yalin_project`], [`yalin_parse_tree`]) parse the caller's bytes in
//!   place without copying them; [`yalin_document_parse`] copies them into the
//!   document it returns.
//! - A document is a `Box` the caller owns and frees exactly once with
//!   [`yalin_document_free`]. It is immutable, so queries may run concurrently
//!   from several threads. The Swift wrapper ties it to a class `deinit`, so it
//!   can never be used after it is freed.
//! - Every result is a [`YalinBuffer`] the caller frees with [`yalin_buffer_free`].
//!   Its contents use the formats of [`yalin_wire::codec`] and [`yalin_wire::tree`].
//! - Panics never cross the boundary: they are caught and reported as a null
//!   buffer or [`YALIN_INTERNAL_ERROR`]. Expected outcomes (bad JSON, missing
//!   fields, …) are encoded in the results.

use std::panic::{AssertUnwindSafe, catch_unwind};

use yalin_core::Document;
use yalin_wire::{codec, tree};

/// Status for a panic or a malformed request: never a parse error code, which
/// is always `-(x + 1)` for a non-negative `x` that fits an `i64` shifted by 8.
pub const YALIN_INTERNAL_ERROR: i64 = i64::MIN;

type OwnedDocument = Document<Vec<u8>>;

/// Opaque document handle (`YalinDocument *` in C).
pub struct YalinDocument(OwnedDocument);

/// Bytes allocated by Rust. `ptr` is null only for an internal error.
#[repr(C)]
pub struct YalinBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl YalinBuffer {
    const NULL: Self = Self { ptr: std::ptr::null_mut(), len: 0, cap: 0 };

    fn from_vec(v: Vec<u8>) -> Self {
        // Keep the capacity instead of shrinking, which could copy the buffer.
        let mut v = std::mem::ManuallyDrop::new(v);
        Self { ptr: v.as_mut_ptr(), len: v.len(), cap: v.capacity() }
    }
}

/// # Safety
/// `ptr` must be valid for `len` bytes for the duration of the call (any value when `len == 0`).
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

fn guarded(f: impl FnOnce() -> Option<Vec<u8>>) -> YalinBuffer {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Some(v)) => YalinBuffer::from_vec(v),
        _ => YalinBuffer::NULL,
    }
}

/// Parses `len` bytes at `input` (copied) into a document stored in `*out`.
///
/// Returns 0 on success, a negative parse error code
/// ([`yalin_wire::codec::parse_error_code`]) for invalid JSON, or
/// [`YALIN_INTERNAL_ERROR`]. `*out` is set to null on any failure.
///
/// # Safety
/// `input` must be valid for `len` bytes and `out` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yalin_document_parse(input: *const u8, len: usize, out: *mut *mut YalinDocument) -> i64 {
    if out.is_null() {
        return YALIN_INTERNAL_ERROR;
    }
    // SAFETY: `out` is valid per the contract.
    unsafe { *out = std::ptr::null_mut() };
    // SAFETY: guaranteed by the caller.
    let input = unsafe { bytes(input, len) };
    let result = catch_unwind(|| Document::parse(input.to_vec()));
    match result {
        Ok(Ok(doc)) => {
            // SAFETY: as above.
            unsafe { *out = Box::into_raw(Box::new(YalinDocument(doc))) };
            0
        }
        Ok(Err(e)) => codec::parse_error_code(&e),
        Err(_) => YALIN_INTERNAL_ERROR,
    }
}

/// Looks up the framed `paths` (`[u32 len][UTF-8]`…) in `doc`.
///
/// # Safety
/// `doc` must come from [`yalin_document_parse`] and not be freed yet;
/// `paths` must be valid for `paths_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yalin_document_query(
    doc: *const YalinDocument,
    paths: *const u8,
    paths_len: usize,
) -> YalinBuffer {
    // SAFETY: a live document per the contract; shared access is sound because it is immutable.
    let Some(doc) = (unsafe { doc.as_ref() }) else { return YalinBuffer::NULL };
    // SAFETY: guaranteed by the caller.
    let paths = unsafe { bytes(paths, paths_len) };
    guarded(|| codec::query(&doc.0, paths).ok())
}

/// Frees a document. Null is ignored.
///
/// # Safety
/// `doc` must be null or come from [`yalin_document_parse`], and is freed at most once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yalin_document_free(doc: *mut YalinDocument) {
    if !doc.is_null() {
        // SAFETY: ownership is handed back exactly once per the contract.
        drop(unsafe { Box::from_raw(doc) });
    }
}

/// One-shot parse and projection of the framed `paths`, reading `input` in place.
///
/// # Safety
/// `input` and `paths` must be valid for their lengths during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yalin_project(
    input: *const u8,
    len: usize,
    paths: *const u8,
    paths_len: usize,
) -> YalinBuffer {
    // SAFETY: guaranteed by the caller.
    let (input, paths) = unsafe { (bytes(input, len), bytes(paths, paths_len)) };
    guarded(|| match Document::parse(input) {
        Ok(doc) => codec::query(&doc, paths).ok(),
        Err(e) => Some(codec::parse_failure(&e)),
    })
}

/// One-shot parse into the encoded tree ([`yalin_wire::tree`]), reading `input` in place.
///
/// # Safety
/// `input` must be valid for `len` bytes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yalin_parse_tree(input: *const u8, len: usize) -> YalinBuffer {
    // SAFETY: guaranteed by the caller.
    let input = unsafe { bytes(input, len) };
    guarded(|| Some(tree::encode(input)))
}

/// Frees a buffer returned by this library. A null buffer is ignored.
///
/// # Safety
/// `buffer` must come from this library, unmodified, and is freed at most once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yalin_buffer_free(buffer: YalinBuffer) {
    if !buffer.ptr.is_null() {
        // SAFETY: the fields are exactly those of a leaked `Vec<u8>` per the contract.
        drop(unsafe { Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.cap) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(paths: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for p in paths {
            codec::put_bytes(&mut out, p.as_bytes());
        }
        out
    }

    /// Copies a buffer out and frees it.
    fn take(buffer: YalinBuffer) -> Option<Vec<u8>> {
        let copy = (!buffer.ptr.is_null()).then(|| unsafe { bytes(buffer.ptr, buffer.len) }.to_vec());
        unsafe { yalin_buffer_free(buffer) };
        copy
    }

    #[test]
    fn document_lifecycle() {
        let input = br#"{"user":{"name":"Ada"}}"#;
        let mut doc = std::ptr::null_mut();
        assert_eq!(unsafe { yalin_document_parse(input.as_ptr(), input.len(), &mut doc) }, 0);
        assert!(!doc.is_null());
        let paths = frame(&["user.name"]);
        let out = take(unsafe { yalin_document_query(doc, paths.as_ptr(), paths.len()) }).unwrap();
        assert_eq!(&out[..8], &[0; 8]);
        assert_eq!(&out[8..], [&[codec::STRING, 3, 0, 0, 0][..], b"Ada"].concat());
        unsafe { yalin_document_free(doc) };
        unsafe { yalin_document_free(std::ptr::null_mut()) };
    }

    #[test]
    fn parse_failure_and_empty_input() {
        let mut doc = std::ptr::null_mut();
        let code = unsafe { yalin_document_parse(b"[1,]".as_ptr(), 4, &mut doc) };
        assert!(code < 0 && code != YALIN_INTERNAL_ERROR);
        assert!(doc.is_null());
        // Null with length 0 is the empty input, an EOF error.
        let code = unsafe { yalin_document_parse(std::ptr::null(), 0, &mut doc) };
        assert_eq!(-(code + 1) & 0xFF, 1);
        assert_eq!(unsafe { yalin_document_parse(b"1".as_ptr(), 1, std::ptr::null_mut()) }, YALIN_INTERNAL_ERROR);
    }

    #[test]
    fn one_shot_calls_match_wire_encoders() {
        let input = br#"{"a":[1,2],"b":"x"}"#;
        let paths = frame(&["a[1]", "b"]);
        let out = take(unsafe { yalin_project(input.as_ptr(), input.len(), paths.as_ptr(), paths.len()) }).unwrap();
        let doc = Document::parse(input.as_slice()).unwrap();
        assert_eq!(out, codec::query(&doc, &paths).unwrap());
        let out = take(unsafe { yalin_parse_tree(input.as_ptr(), input.len()) }).unwrap();
        assert_eq!(out, tree::encode(input));
    }

    #[test]
    fn bad_framing_and_null_document_are_internal_errors() {
        let input = b"{}";
        assert!(take(unsafe { yalin_project(input.as_ptr(), 2, [1u8, 0].as_ptr(), 2) }).is_none());
        assert!(take(unsafe { yalin_document_query(std::ptr::null(), std::ptr::null(), 0) }).is_none());
    }
}
