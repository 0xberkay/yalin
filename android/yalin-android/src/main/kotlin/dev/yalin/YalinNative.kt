package dev.yalin

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Raw JNI surface of `libyalin.so` (crates/yalin-jni). Wire formats are
 * documented in `crates/yalin-jni/src/codec.rs`; keep both sides in sync.
 *
 * Input bytes are copied into native memory during each call, so callers may
 * reuse or mutate their arrays afterwards.
 */
internal object YalinNative {
  init {
    System.loadLibrary("yalin")
  }

  /** Positive document handle, or a negative parse error code. */
  @JvmStatic external fun parse(input: ByteArray): Long

  /** Like [parse], reading `length` bytes at `offset` of a direct buffer. */
  @JvmStatic external fun parseDirect(buffer: ByteBuffer, offset: Int, length: Int): Long

  /** Encoded results, or `null` if the handle is closed or invalid. */
  @JvmStatic external fun query(handle: Long, paths: ByteArray): ByteArray?

  /** One-shot parse + query; encoded results (status carries parse errors). */
  @JvmStatic external fun project(input: ByteArray, paths: ByteArray): ByteArray

  /** Idempotent. */
  @JvmStatic external fun close(handle: Long)

  /** Frames paths as repeated `[u32 LE length][UTF-8 bytes]`. */
  fun encodePaths(paths: Array<out String>): ByteArray {
    val encoded = Array(paths.size) { paths[it].encodeToByteArray() }
    val out = ByteBuffer.allocate(encoded.sumOf { 4 + it.size }).order(ByteOrder.LITTLE_ENDIAN)
    for (bytes in encoded) {
      out.putInt(bytes.size)
      out.put(bytes)
    }
    return out.array()
  }
}
