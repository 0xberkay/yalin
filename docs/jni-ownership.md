# JNI ownership and lifecycle

Code: `crates/yalin-jni` (Rust side), `android/yalin-android/src/main/kotlin/dev/yalin` (Kotlin side).

## Input bytes

- `ByteArray` and `ByteBuffer` input is **copied** into Rust-owned memory during the JNI call.
  No pointer into a Java array or buffer outlives the call, so JVM pin/copy behaviour and GC
  moves don't matter. The caller can reuse or mutate its array as soon as the call returns.
- Direct `ByteBuffer`: the bytes between `position` and `limit` are copied natively, with no
  intermediate Java array. The caller must not mutate that region *during* the call. The
  buffer's position is not changed.
- Heap `ByteBuffer`: copied into a `ByteArray` first.
- Zero-copy borrowing of Java memory is deliberately not offered. Revisit it only if
  benchmarks show the copy matters.

## Document handles

- A parsed document is owned by a process-wide registry (`registry.rs`). Kotlin holds an
  opaque positive `Long`, `(generation << 32) | slot`, never a pointer.
- A closed, stale or forged handle resolves to nothing. The slot's generation changes on every
  close, so reusing a slot never revives an old handle.
- Lookups clone an `Arc` and release the registry lock before doing any work. `close()`
  racing a query is therefore safe: the document is freed after the last in-flight query.
- `YalinDocument` is `Closeable`. `close()` is idempotent. A query after `close()` throws
  `IllegalStateException`.
- No `Cleaner`/finalizer yet: an unclosed document leaks until process exit.
  (`java.lang.ref.Cleaner` needs API 33; minSdk is 24.)

## Errors

| Situation | Kotlin sees |
|---|---|
| Invalid JSON / limit exceeded | `YalinParseException(error, offset)`, decoded from a negative return code. Nothing is thrown from native code. |
| Malformed path | `YalinPathException(path, position)` |
| Wrong type (value or mid-path) | `YalinTypeException(path, expected, found)` |
| Number not representable | `ArithmeticException` |
| Missing / JSON `null` | `null` from getters; `kind()` tells them apart |
| Closed document | `IllegalStateException` |
| Rust panic or JNI failure | `RuntimeException` (panics are caught at the boundary by `jni` `with_env`) |

Release builds keep `panic = "unwind"` so that panics can be caught at the boundary.

## Call granularity

One JNI call per `project(...)`, whatever the number of paths. Results come back as a single
`ByteArray` (format in `codec.rs`). Kotlin decodes it lazily, so a string is only created when
it is read.
