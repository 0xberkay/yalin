package dev.yalin

import java.math.BigDecimal

/**
 * An immutable JSON value from [YalinJson.parseTree].
 *
 * An integer that fits a `Long` is [YalinLong],
 * any other integer is [YalinBigNumber], a number with a fraction or exponent
 * is [YalinDouble] unless it overflows `Double`, then it is [YalinBigNumber].
 */
sealed interface YalinValue

object YalinNull : YalinValue {
  override fun toString() = "null"
}

class YalinBoolean private constructor(val value: Boolean) : YalinValue {
  override fun toString() = value.toString()

  companion object {
    @JvmField val TRUE = YalinBoolean(true)
    @JvmField val FALSE = YalinBoolean(false)

    @JvmStatic fun of(value: Boolean) = if (value) TRUE else FALSE
  }
}

data class YalinString(val value: String) : YalinValue

data class YalinLong(val value: Long) : YalinValue {
  companion object {
    private const val CACHE_LOW = -128L
    private const val CACHE_HIGH = 1023L
    private val cache = Array((CACHE_HIGH - CACHE_LOW + 1).toInt()) { YalinLong(it + CACHE_LOW) }

    /** A shared instance for small values, like `java.lang.Long.valueOf`. */
    @JvmStatic
    fun of(value: Long): YalinLong =
      if (value in CACHE_LOW..CACHE_HIGH) cache[(value - CACHE_LOW).toInt()] else YalinLong(value)
  }
}

data class YalinDouble(val value: Double) : YalinValue

/** A number outside the range of `Long` or finite `Double`; see [YalinValue]. */
data class YalinBigNumber(val value: BigDecimal) : YalinValue

/** A JSON array. Elements are in document order. */
class YalinArray internal constructor(private val items: Array<YalinValue>) :
  AbstractList<YalinValue>(), RandomAccess, YalinValue {
  override val size: Int
    get() = items.size

  override fun get(index: Int): YalinValue = items[index]
}

/**
 * A JSON object. Keys are unique and iterate in document order. Lookups scan
 * small objects and use a hash index, built on first use, for larger ones.
 */
class YalinObject internal constructor(
  /** Keys at even indices, each followed by its value; one array per object keeps allocation low. */
  private val members: Array<Any>,
) : AbstractMap<String, YalinValue>(), YalinValue {
  @Volatile private var index: HashMap<String, Int>? = null

  override val size: Int
    get() = members.size / 2

  override fun get(key: String): YalinValue? = indexOf(key).let { if (it < 0) null else valueAt(it) }

  override fun containsKey(key: String): Boolean = indexOf(key) >= 0

  // A view created on access, so building a tree allocates nothing per object for it.
  override val entries: Set<Map.Entry<String, YalinValue>>
    get() =
      object : AbstractSet<Map.Entry<String, YalinValue>>() {
        override val size: Int
          get() = this@YalinObject.size

        override fun iterator(): Iterator<Map.Entry<String, YalinValue>> =
          (0 until size).asSequence().map { Entry(keyAt(it), valueAt(it)) }.iterator()
      }

  private fun keyAt(i: Int) = members[2 * i] as String

  private fun valueAt(i: Int) = members[2 * i + 1] as YalinValue

  /** Member index of [key], or -1. */
  private fun indexOf(key: String): Int {
    val n = size
    if (n <= LINEAR_KEYS) {
      for (i in 0 until n) if (members[2 * i] == key) return i
      return -1
    }
    val map =
      index
        ?: HashMap<String, Int>(n * 2).also { m ->
          for (i in 0 until n) m[keyAt(i)] = i
          index = m
        }
    return map[key] ?: -1
  }

  private class Entry(override val key: String, override val value: YalinValue) :
    Map.Entry<String, YalinValue> {
    override fun equals(other: Any?) = other is Map.Entry<*, *> && key == other.key && value == other.value

    override fun hashCode() = key.hashCode() xor value.hashCode()

    override fun toString() = "$key=$value"
  }

  private companion object {
    const val LINEAR_KEYS = 8
  }
}
