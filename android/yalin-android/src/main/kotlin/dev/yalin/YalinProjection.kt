package dev.yalin

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Results of looking up several paths in one native call, indexed like the
 * requested paths. Decoding is lazy: strings are only created when read.
 *
 * Accessors return `null` for both a missing path and a JSON `null`; use
 * [kind] to tell them apart. A value of another type throws
 * [YalinTypeException]; a malformed path throws [YalinPathException].
 */
class YalinProjection internal constructor(private val paths: Array<out String>, data: ByteArray) {
  private val buf = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
  private val offsets = IntArray(paths.size)

  init {
    val status = buf.getLong(0)
    if (status != 0L) throw YalinParseException.fromCode(status)
    var at = 8
    for (i in paths.indices) {
      offsets[i] = at
      at += entrySize(at)
    }
  }

  val size: Int
    get() = paths.size

  fun kind(index: Int): YalinKind =
    when (tag(index)) {
      MISSING -> YalinKind.MISSING
      NULL -> YalinKind.NULL
      FALSE, TRUE -> YalinKind.BOOLEAN
      INT, DOUBLE, BIG_NUMBER -> YalinKind.NUMBER
      STRING -> YalinKind.STRING
      ARRAY -> YalinKind.ARRAY
      else -> YalinKind.OBJECT
    }

  fun getString(index: Int): String? =
    when (tag(index)) {
      MISSING, NULL -> null
      STRING -> {
        val at = offsets[index] + 1
        String(buf.array(), at + 4, buf.getInt(at), Charsets.UTF_8)
      }
      else -> mismatch(index, YalinKind.STRING)
    }

  /** @throws ArithmeticException if the number is fractional or outside the `Long` range. */
  fun getLong(index: Int): Long? =
    when (tag(index)) {
      MISSING, NULL -> null
      INT -> buf.getLong(offsets[index] + 1)
      DOUBLE, BIG_NUMBER -> throw ArithmeticException("'${paths[index]}' is not representable as Long")
      else -> mismatch(index, YalinKind.NUMBER)
    }

  /** @throws ArithmeticException if the number overflows `Double`. */
  fun getDouble(index: Int): Double? =
    when (tag(index)) {
      MISSING, NULL -> null
      INT -> buf.getLong(offsets[index] + 1).toDouble()
      DOUBLE -> buf.getDouble(offsets[index] + 1)
      BIG_NUMBER -> throw ArithmeticException("'${paths[index]}' is not representable as Double")
      else -> mismatch(index, YalinKind.NUMBER)
    }

  fun getBoolean(index: Int): Boolean? =
    when (tag(index)) {
      MISSING, NULL -> null
      TRUE -> true
      FALSE -> false
      else -> mismatch(index, YalinKind.BOOLEAN)
    }

  /** Tag of a resolvable entry; path and intermediate-type errors throw here. */
  private fun tag(index: Int): Byte {
    val at = offsets[index]
    return when (val tag = buf.get(at)) {
      INVALID_PATH -> throw YalinPathException(paths[index], buf.getInt(at + 1))
      TYPE_MISMATCH ->
        throw YalinTypeException(
          paths[index],
          YalinKind.entries[buf.get(at + 1).toInt()],
          YalinKind.entries[buf.get(at + 2).toInt()],
        )
      else -> tag
    }
  }

  private fun mismatch(index: Int, expected: YalinKind): Nothing =
    throw YalinTypeException(paths[index], expected, kind(index))

  private fun entrySize(at: Int): Int =
    when (buf.get(at)) {
      INT, DOUBLE -> 9
      BIG_NUMBER, STRING -> 5 + buf.getInt(at + 1)
      INVALID_PATH -> 5
      TYPE_MISMATCH -> 3
      else -> 1
    }

  private companion object {
    // Tags from crates/yalin-wire/src/codec.rs.
    const val MISSING: Byte = 0
    const val NULL: Byte = 1
    const val FALSE: Byte = 2
    const val TRUE: Byte = 3
    const val INT: Byte = 4
    const val DOUBLE: Byte = 5
    const val BIG_NUMBER: Byte = 6
    const val STRING: Byte = 7
    const val ARRAY: Byte = 8
    const val INVALID_PATH: Byte = 10
    const val TYPE_MISMATCH: Byte = 11
  }
}
