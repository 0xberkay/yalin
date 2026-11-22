// C interface of libyalin_ffi (crates/yalin-ffi/src/lib.rs). Keep both in sync.
// Result buffers use the formats documented in crates/yalin-wire.
#ifndef YALIN_H
#define YALIN_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Status for a panic or a malformed request; never a parse error code.
#define YALIN_INTERNAL_ERROR INT64_MIN

/// Opaque, immutable parsed document. Safe to query from several threads.
typedef struct YalinDocument YalinDocument;

/// Bytes owned by the library; free with yalin_buffer_free. `ptr` is null on an internal error.
typedef struct YalinBuffer {
  uint8_t *ptr;
  size_t len;
  size_t cap;
} YalinBuffer;

/// Parses (and copies) `len` bytes. Returns 0 and sets `*out`, or a negative
/// parse error code / YALIN_INTERNAL_ERROR and sets `*out` to NULL.
int64_t yalin_document_parse(const uint8_t *input, size_t len, YalinDocument **out);

/// Looks up framed paths (`[u32 len][UTF-8]`...) in a live document.
YalinBuffer yalin_document_query(const YalinDocument *doc, const uint8_t *paths, size_t paths_len);

/// Frees a document (NULL is ignored). Call at most once per document.
void yalin_document_free(YalinDocument *doc);

/// One-shot parse + projection; `input` is read in place and not retained.
YalinBuffer yalin_project(const uint8_t *input, size_t len, const uint8_t *paths, size_t paths_len);

/// One-shot parse into the encoded tree; `input` is read in place and not retained.
YalinBuffer yalin_parse_tree(const uint8_t *input, size_t len);

/// Frees a buffer returned by this library (a null buffer is ignored).
void yalin_buffer_free(YalinBuffer buffer);

#ifdef __cplusplus
}
#endif

#endif
