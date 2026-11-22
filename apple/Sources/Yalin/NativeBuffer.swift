import Foundation
import YalinFFI

/// Owns a result buffer allocated by Rust and frees it on `deinit`.
/// Readers decode straight from native memory, with no copy into Swift.
final class NativeBuffer: @unchecked Sendable {
  // Immutable after init, so sharing between threads is safe.
  private let raw: YalinBuffer
  let base: UnsafeRawPointer
  let count: Int

  /// Takes ownership of `raw`; throws ``YalinError/internal`` for a null buffer.
  init(_ raw: YalinBuffer) throws {
    guard let ptr = raw.ptr, raw.len >= 8 else {
      yalin_buffer_free(raw)
      throw YalinError.internal
    }
    self.raw = raw
    self.base = UnsafeRawPointer(ptr)
    self.count = raw.len
  }

  deinit { yalin_buffer_free(raw) }

  /// The leading `i64` status: 0, or a parse error code.
  var status: Int64 { base.readInt64(at: 0) }
}

extension UnsafeRawPointer {
  @inline(__always) func readInt64(at offset: Int) -> Int64 {
    Int64(littleEndian: loadUnaligned(fromByteOffset: offset, as: Int64.self))
  }

  @inline(__always) func readUInt32(at offset: Int) -> Int {
    Int(UInt32(littleEndian: loadUnaligned(fromByteOffset: offset, as: UInt32.self)))
  }

  @inline(__always) func readUInt16(at offset: Int) -> Int {
    Int(UInt16(littleEndian: loadUnaligned(fromByteOffset: offset, as: UInt16.self)))
  }

  @inline(__always) func readDouble(at offset: Int) -> Double {
    Double(bitPattern: UInt64(littleEndian: loadUnaligned(fromByteOffset: offset, as: UInt64.self)))
  }

  /// `count` bytes at `offset`, which the native parser validated as UTF-8.
  @inline(__always) func readString(at offset: Int, count: Int) -> String {
    String(decoding: UnsafeRawBufferPointer(start: self + offset, count: count), as: UTF8.self)
  }
}

/// Encodes paths as `[u32 len][UTF-8]`... (see crates/yalin-wire/src/codec.rs).
func framePaths(_ paths: [String]) -> [UInt8] {
  var size = 0
  for path in paths { size += 4 + path.utf8.count }
  var out = [UInt8]()
  out.reserveCapacity(size)
  for path in paths {
    let len = UInt32(truncatingIfNeeded: path.utf8.count)
    out.append(UInt8(truncatingIfNeeded: len))
    out.append(UInt8(truncatingIfNeeded: len >> 8))
    out.append(UInt8(truncatingIfNeeded: len >> 16))
    out.append(UInt8(truncatingIfNeeded: len >> 24))
    out.append(contentsOf: path.utf8)
  }
  return out
}

/// Calls `body` with a pointer and length for raw bytes, as the C API expects.
@inline(__always)
func withBytes<R>(_ bytes: some ContiguousBytes, _ body: (UnsafePointer<UInt8>?, Int) throws -> R) rethrows -> R {
  try bytes.withUnsafeBytes { raw in
    try body(raw.baseAddress?.assumingMemoryBound(to: UInt8.self), raw.count)
  }
}

/// Like the `ContiguousBytes` variant, for the UTF-8 form of `string` (no copy for native strings).
@inline(__always)
func withBytes<R>(_ string: String, _ body: (UnsafePointer<UInt8>?, Int) throws -> R) rethrows -> R {
  var string = string
  return try string.withUTF8 { try body($0.baseAddress, $0.count) }
}
