package dev.yalin

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * A validated JSON document held in native memory. Always [close] it (or use
 * `use {}`); an unclosed document stays allocated until the process exits.
 *
 * Thread-safe: queries may run concurrently from several threads, and
 * [close] may race them safely. A query after (or losing the race with)
 * [close] throws [IllegalStateException].
 *
 * Path syntax: `user.name`, `items[3].price`, `[0]`; see yalin-core `query` docs.
 */
class YalinDocument internal constructor(handle: Long) : Closeable {
  private val handle = AtomicLong(handle)

  /** Looks up all [paths] in one native call. */
  fun project(vararg paths: String): YalinProjection {
    val h = handle.get()
    val data = if (h == 0L) null else YalinNative.query(h, YalinNative.encodePaths(paths))
    checkNotNull(data) { "YalinDocument is closed" }
    return YalinProjection(paths, data)
  }

  fun kind(path: String): YalinKind = project(path).kind(0)

  fun getString(path: String): String? = project(path).getString(0)

  fun getLong(path: String): Long? = project(path).getLong(0)

  fun getDouble(path: String): Double? = project(path).getDouble(0)

  fun getBoolean(path: String): Boolean? = project(path).getBoolean(0)

  override fun close() {
    val h = handle.getAndSet(0)
    if (h != 0L) YalinNative.close(h)
  }
}
