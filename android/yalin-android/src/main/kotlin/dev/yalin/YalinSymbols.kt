package dev.yalin

/**
 * Process-wide cache of short ASCII keys and string values, so that a tree
 * reuses the `String` and [YalinString] objects of earlier trees instead of
 * creating them again (on ART that costs far more than a lookup). Entries are
 * direct-mapped by hash; a collision replaces the older entry, so memory stays
 * bounded at [SIZE] entries per table.
 *
 * Thread-safe without locks: entries are immutable objects, so a racy read sees
 * either a complete entry or another one, and every hit is verified byte by byte.
 */
internal object YalinSymbols {
  private const val SIZE = 4096
  private val keys = arrayOfNulls<String>(SIZE)
  private val strings = arrayOfNulls<YalinString>(SIZE)

  /** The cached key equal to `data[at until at + len]`, or `null`; [slotOf] gives `slot`. */
  fun key(slot: Int, data: ByteArray, at: Int, len: Int): String? =
    keys[slot]?.takeIf { matches(it, data, at, len) }

  fun putKey(slot: Int, key: String) {
    keys[slot] = key
  }

  fun string(slot: Int, data: ByteArray, at: Int, len: Int): YalinString? =
    strings[slot]?.takeIf { matches(it.value, data, at, len) }

  fun putString(slot: Int, value: YalinString) {
    strings[slot] = value
  }

  /** Table slot for the bytes, or -1 if they are not all ASCII (those are never cached). */
  fun slotOf(data: ByteArray, at: Int, len: Int): Int {
    var h = len
    for (i in at until at + len) {
      val b = data[i].toInt()
      if (b < 0) return -1
      h = 31 * h + b
    }
    return (h xor (h ushr 13)) and (SIZE - 1)
  }

  /** [s] equals the ASCII bytes. */
  private fun matches(s: String, data: ByteArray, at: Int, len: Int): Boolean {
    if (s.length != len) return false
    for (i in 0 until len) if (s[i].code != data[at + i].toInt()) return false
    return true
  }
}
