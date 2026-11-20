package dev.yalin

import java.math.BigDecimal

/** Decodes the tree format of crates/yalin-jni/src/tree.rs. Keep both in sync. */
internal class YalinTreeDecoder private constructor(private val data: ByteArray) {
  private var pos = 0

  /** Mirrors of the encoder's slot caches, filled by `*_NEW` entries; sized by the header. */
  private var keySlots = NO_KEYS
  private var stringSlots = NO_STRINGS
  /** Scratch for [string], grown on demand. */
  private var chars = CharArray(64)

  private fun value(): YalinValue =
    when (data[pos++].toInt()) {
      NULL -> YalinNull
      FALSE -> YalinBoolean.FALSE
      TRUE -> YalinBoolean.TRUE
      INT -> YalinLong.of(long())
      DOUBLE -> YalinDouble(Double.fromBits(long()))
      BIG_NUMBER -> YalinBigNumber(BigDecimal(string(int())))
      STRING -> YalinString(string(int()))
      STRING_NEW -> {
        val slot = slot()
        sharedString(short()).also { stringSlots[slot] = it }
      }
      STRING_REF -> stringSlots[slot()]!!
      ARRAY -> {
        val items = arrayOfNulls<YalinValue>(int())
        for (i in items.indices) items[i] = value()
        @Suppress("UNCHECKED_CAST")
        YalinArray(items as Array<YalinValue>)
      }
      OBJECT -> {
        val members = arrayOfNulls<Any>(2 * int())
        var i = 0
        while (i < members.size) {
          members[i] = key()
          members[i + 1] = value()
          i += 2
        }
        @Suppress("UNCHECKED_CAST")
        YalinObject(members as Array<Any>)
      }
      else -> error("corrupt tree at byte ${pos - 1}")
    }

  private fun key(): String =
    when (data[pos++].toInt()) {
      KEY -> string(int())
      KEY_NEW -> {
        val slot = slot()
        sharedKey(short()).also { keySlots[slot] = it }
      }
      KEY_REF -> keySlots[slot()]!!
      else -> error("corrupt key at byte ${pos - 1}")
    }

  private fun short(): Int = data[pos++].toInt() and 0xFF

  /** A short key, from [YalinSymbols] when an earlier tree had it. */
  private fun sharedKey(len: Int): String {
    val shared = YalinSymbols.slotOf(data, pos, len)
    if (shared >= 0) {
      YalinSymbols.key(shared, data, pos, len)?.let {
        pos += len
        return it
      }
    }
    return string(len).also { if (shared >= 0) YalinSymbols.putKey(shared, it) }
  }

  /** Like [sharedKey], for short string values. */
  private fun sharedString(len: Int): YalinString {
    val shared = YalinSymbols.slotOf(data, pos, len)
    if (shared >= 0) {
      YalinSymbols.string(shared, data, pos, len)?.let {
        pos += len
        return it
      }
    }
    return YalinString(string(len)).also { if (shared >= 0) YalinSymbols.putString(shared, it) }
  }

  /**
   * Decodes [len] bytes at [pos], which the native parser validated as UTF-8.
   * On ART this is faster than `String(bytes, UTF_8)`, which also allocates
   * scratch arrays per call, and, for ASCII, than the high-byte `String` constructor.
   */
  private fun string(len: Int): String {
    // UTF-16 never needs more units than UTF-8 has bytes.
    if (chars.size < len) chars = CharArray(maxOf(len, 2 * chars.size))
    val c = chars
    val d = data
    var i = pos
    val end = pos + len
    var n = 0
    while (i < end) {
      val b = d[i].toInt()
      when {
        b >= 0 -> {
          c[n++] = b.toChar()
          i += 1
        }
        b < -32 -> { // 110xxxxx
          c[n++] = (((b and 0x1F) shl 6) or (d[i + 1].toInt() and 0x3F)).toChar()
          i += 2
        }
        b < -16 -> { // 1110xxxx
          c[n++] =
            (((b and 0x0F) shl 12) or ((d[i + 1].toInt() and 0x3F) shl 6) or (d[i + 2].toInt() and 0x3F)).toChar()
          i += 3
        }
        else -> { // 11110xxx: a surrogate pair
          val cp =
            ((b and 0x07) shl 18) or
              ((d[i + 1].toInt() and 0x3F) shl 12) or
              ((d[i + 2].toInt() and 0x3F) shl 6) or
              (d[i + 3].toInt() and 0x3F)
          c[n++] = Char.MIN_HIGH_SURROGATE + ((cp - 0x10000) ushr 10)
          c[n++] = Char.MIN_LOW_SURROGATE + ((cp - 0x10000) and 0x3FF)
          i += 4
        }
      }
    }
    pos = end
    return String(c, 0, n)
  }

  private fun slot(): Int {
    val p = pos
    pos = p + 2
    return (data[p].toInt() and 0xFF) or ((data[p + 1].toInt() and 0xFF) shl 8)
  }

  private fun int(): Int {
    val d = data
    val p = pos
    pos = p + 4
    return (d[p].toInt() and 0xFF) or
      ((d[p + 1].toInt() and 0xFF) shl 8) or
      ((d[p + 2].toInt() and 0xFF) shl 16) or
      (d[p + 3].toInt() shl 24)
  }

  private fun long(): Long = (int().toLong() and 0xFFFF_FFFFL) or (int().toLong() shl 32)

  companion object {
    /** @throws YalinParseException if the status reports a parse or duplicate-key failure. */
    fun decode(data: ByteArray): YalinValue {
      val decoder = YalinTreeDecoder(data)
      val status = decoder.long()
      if (status != 0L) throw YalinParseException.fromCode(status)
      val slots = 1 shl data[decoder.pos++].toInt()
      decoder.keySlots = arrayOfNulls(slots)
      decoder.stringSlots = arrayOfNulls(slots)
      return decoder.value()
    }

    private val NO_KEYS = arrayOfNulls<String>(0)
    private val NO_STRINGS = arrayOfNulls<YalinString>(0)

    // Tags from tree.rs, as Int so that `when` compiles to a table switch.
    private const val NULL = 1
    private const val FALSE = 2
    private const val TRUE = 3
    private const val INT = 4
    private const val DOUBLE = 5
    private const val BIG_NUMBER = 6
    private const val STRING = 7
    private const val ARRAY = 8
    private const val OBJECT = 9
    private const val STRING_NEW = 12
    private const val STRING_REF = 13
    private const val KEY = 0
    private const val KEY_NEW = 1
    private const val KEY_REF = 2
  }
}
