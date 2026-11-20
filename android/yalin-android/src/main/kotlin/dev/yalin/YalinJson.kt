package dev.yalin

import java.nio.ByteBuffer

/** Entry point. All inputs are fully validated before any value is returned. */
object YalinJson {
  /** Parses [input] (copied into native memory) into a document you must close. */
  @JvmStatic
  fun parse(input: ByteArray): YalinDocument = open(YalinNative.parse(input))

  /**
   * Parses the bytes between [ByteBuffer.position] and [ByteBuffer.limit] without
   * changing the buffer's position. A direct buffer is copied natively without
   * an intermediate array (do not modify it during the call); a heap buffer is
   * copied into an array first.
   */
  @JvmStatic
  fun parse(input: ByteBuffer): YalinDocument {
    val code =
      if (input.isDirect) {
        YalinNative.parseDirect(input, input.position(), input.remaining())
      } else {
        YalinNative.parse(ByteArray(input.remaining()).also { input.duplicate().get(it) })
      }
    return open(code)
  }

  /** Parses and looks up [paths] in a single native call, without keeping a document. */
  @JvmStatic
  fun project(input: ByteArray, vararg paths: String): YalinProjection =
    YalinProjection(paths, YalinNative.project(input, YalinNative.encodePaths(paths)))

  /**
   * Parses [input] into an immutable [YalinValue] tree in one native call.
   * Stricter than [parse]: a repeated object key throws [YalinParseException]
   * with [YalinParseError.DUPLICATE_KEY].
   */
  @JvmStatic fun parseTree(input: ByteArray): YalinValue = YalinTreeDecoder.decode(YalinNative.parseTree(input))

  /** Like [parseTree]; [input] is encoded to UTF-8 first, so error offsets are UTF-8 byte offsets. */
  @JvmStatic fun parseTree(input: String): YalinValue = parseTree(input.encodeToByteArray())

  private fun open(code: Long): YalinDocument {
    if (code < 0) throw YalinParseException.fromCode(code)
    return YalinDocument(code)
  }
}
