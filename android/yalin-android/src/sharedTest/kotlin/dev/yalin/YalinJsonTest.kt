package dev.yalin

import java.math.BigDecimal
import java.nio.ByteBuffer
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/** Runs on the host JVM (`test`) and on Android (`androidTest`) against the real libyalin. */
class YalinJsonTest {
  private val bs = '\\'
  private val json =
    """
    {"user": {"id": 42, "name": "Ada", "email": null, "active": true, "score": 9.5,
              "nick": "${bs}u0041da${bs}n", "big": 123456789012345678901, "huge": 1e400},
     "items": [{"price": 1}, {"price": 2}, {"price": 3}, {"price": 4.25}],
     "text": "héllo 中文 😀"}
    """
      .trimIndent()
      .encodeToByteArray()

  @Test
  fun typedGetters() {
    YalinJson.parse(json).use { doc ->
      assertEquals("Ada", doc.getString("user.name"))
      assertEquals("Ada\n", doc.getString("user.nick"))
      assertEquals("héllo 中文 😀", doc.getString("text"))
      assertEquals(42L, doc.getLong("user.id"))
      assertEquals(9.5, doc.getDouble("user.score")!!, 0.0)
      assertEquals(42.0, doc.getDouble("user.id")!!, 0.0)
      assertEquals(4.25, doc.getDouble("items[3].price")!!, 0.0)
      assertEquals(true, doc.getBoolean("user.active"))
      assertEquals(YalinKind.ARRAY, doc.kind("items"))
      assertEquals(YalinKind.OBJECT, doc.kind(""))
    }
  }

  @Test
  fun missingNullAndTypeAreDistinct() {
    YalinJson.parse(json).use { doc ->
      assertEquals(YalinKind.NULL, doc.kind("user.email"))
      assertEquals(YalinKind.MISSING, doc.kind("user.phone"))
      assertNull(doc.getString("user.email"))
      assertNull(doc.getString("user.phone"))
      assertNull(doc.getLong("items[9].price"))
      val e = assertThrows(YalinTypeException::class.java) { doc.getString("user.id") }
      assertEquals(YalinKind.STRING, e.expected)
      assertEquals(YalinKind.NUMBER, e.found)
      val mid = assertThrows(YalinTypeException::class.java) { doc.getLong("items.price") }
      assertEquals(YalinKind.OBJECT, mid.expected)
      assertEquals(YalinKind.ARRAY, mid.found)
      assertThrows(ArithmeticException::class.java) { doc.getLong("user.score") }
      assertThrows(ArithmeticException::class.java) { doc.getLong("user.big") }
      assertThrows(ArithmeticException::class.java) { doc.getDouble("user.huge") }
      val p = assertThrows(YalinPathException::class.java) { doc.getString("user..name") }
      assertEquals(5, p.position)
    }
  }

  @Test
  fun projection() {
    YalinJson.parse(json).use { doc ->
      val r = doc.project("user.id", "user.name", "user.active", "user.phone", "user.email")
      assertEquals(5, r.size)
      assertEquals(42L, r.getLong(0))
      assertEquals("Ada", r.getString(1))
      assertEquals(true, r.getBoolean(2))
      assertEquals(YalinKind.MISSING, r.kind(3))
      assertEquals(YalinKind.NULL, r.kind(4))
    }
    val oneShot = YalinJson.project(json, "items[0].price", "items[2].price", "nope")
    assertEquals(1L, oneShot.getLong(0))
    assertEquals(3L, oneShot.getLong(1))
    assertEquals(YalinKind.MISSING, oneShot.kind(2))
    assertEquals(0, YalinJson.project(json).size)
  }

  @Test
  fun parseErrors() {
    val e = assertThrows(YalinParseException::class.java) { YalinJson.parse("[1,  ]".encodeToByteArray()) }
    assertEquals(YalinParseError.UNEXPECTED_CHARACTER, e.error)
    assertEquals(5L, e.offset)
    val lone = assertThrows(YalinParseException::class.java) {
      YalinJson.project("[\"${bs}uD800\"]".encodeToByteArray(), "[0]")
    }
    assertEquals(YalinParseError.INVALID_SURROGATE, lone.error)
    assertEquals(
      YalinParseError.UNEXPECTED_EOF,
      assertThrows(YalinParseException::class.java) { YalinJson.parse(ByteArray(0)) }.error,
    )
    val deep = "[".repeat(5000) + "]".repeat(5000)
    assertEquals(
      YalinParseError.DEPTH_LIMIT_EXCEEDED,
      assertThrows(YalinParseException::class.java) { YalinJson.parse(deep.encodeToByteArray()) }.error,
    )
  }

  @Test
  fun treeHasEveryValue() {
    val root = YalinJson.parseTree(json) as YalinObject
    assertEquals(listOf("user", "items", "text"), root.keys.toList())
    val user = root["user"] as YalinObject
    assertEquals(YalinLong(42), user["id"])
    assertEquals(YalinString("Ada"), user["name"])
    assertEquals(YalinNull, user["email"])
    assertEquals(YalinBoolean.TRUE, user["active"])
    assertEquals(YalinDouble(9.5), user["score"])
    assertEquals(YalinString("Ada\n"), user["nick"])
    assertEquals(YalinBigNumber(BigDecimal("123456789012345678901")), user["big"])
    assertEquals(YalinBigNumber(BigDecimal("1e400")), user["huge"])
    assertNull(user["phone"])
    val items = root["items"] as YalinArray
    assertEquals(4, items.size)
    assertEquals(YalinDouble(4.25), (items[3] as YalinObject)["price"])
    assertEquals(YalinString("héllo 中文 😀"), root["text"])
    assertEquals(YalinJson.parseTree(String(json, Charsets.UTF_8)), root)
  }

  @Test
  fun treeDecodesEveryUtf8Width() {
    val text = "a é ı ş 中文 € 😀 \u0000 \u07FF \u0800 \uFFFF ${String(Character.toChars(0x10FFFF))}"
    val json = "[\"" + text.replace("\u0000", "\\u0000") + "\"]"
    assertEquals(listOf(YalinString(text)), YalinJson.parseTree(json))
    val long = "çğ".repeat(500) // longer than the decoder's initial scratch
    assertEquals(YalinString(long), YalinJson.parseTree("\"$long\""))
  }

  @Test
  fun treeRoots() {
    assertEquals(YalinLong(-7), YalinJson.parseTree("-7"))
    assertEquals(YalinDouble(1e2), YalinJson.parseTree("1e2"))
    assertEquals(YalinLong(Long.MIN_VALUE), YalinJson.parseTree(Long.MIN_VALUE.toString()))
    assertEquals(YalinString(""), YalinJson.parseTree("\"\""))
    assertEquals(YalinBoolean.FALSE, YalinJson.parseTree(" false "))
    assertEquals(emptyList<YalinValue>(), YalinJson.parseTree("[]"))
    assertEquals(emptyMap<String, YalinValue>(), YalinJson.parseTree("{}"))
  }

  @Test
  fun bigObjectLookupAndOrder() {
    val keys = (0 until 100).map { "k$it" }
    val tree = YalinJson.parseTree(keys.joinToString(",", "{", "}") { "\"$it\":${it.drop(1)}" }) as YalinObject
    assertEquals(keys, tree.keys.toList())
    for ((i, k) in keys.withIndex()) assertEquals(YalinLong(i.toLong()), tree[k])
    assertFalse(tree.containsKey("k100"))
    assertEquals(tree, keys.withIndex().associate { (i, k) -> k to YalinLong(i.toLong()) })
  }

  @Test
  fun repeatedAndCollidingStrings() {
    // Far more distinct short keys and values than cache slots, each seen twice.
    val words = (0 until 5000).map { "w${it % 2500}" }
    val json = words.joinToString(",", "[", "]") { "{\"$it\":\"$it\",\"x\":\"${"y".repeat(40)}\"}" }
    val tree = YalinJson.parseTree(json) as YalinArray
    for ((i, w) in words.withIndex()) {
      val o = tree[i] as YalinObject
      assertEquals(listOf(w, "x"), o.keys.toList())
      assertEquals(YalinString(w), o[w])
      assertEquals(YalinString("y".repeat(40)), o["x"])
    }
  }

  @Test
  fun sharedSymbolsAcrossTrees() {
    val a = YalinJson.parseTree("""{"status":"ok","n":"é"}""") as YalinObject
    val b = YalinJson.parseTree("""{"status":"ok","n":"é"}""") as YalinObject
    assertTrue(a.keys.first() === b.keys.first())
    assertTrue(a["status"] === b["status"])
    assertFalse(a["n"] === b["n"]) // non-ASCII is never shared
    // Distinct strings that may share a table slot still decode to themselves.
    repeat(3) {
      for (i in 0 until 10_000) {
        val t = YalinJson.parseTree("{\"k$i\":\"v$i\"}") as YalinObject
        assertEquals(YalinString("v$i"), t["k$i"])
      }
    }
  }

  @Test
  fun treeErrors() {
    val dup = assertThrows(YalinParseException::class.java) { YalinJson.parseTree("""{"a":1,"${bs}u0061":2}""") }
    assertEquals(YalinParseError.DUPLICATE_KEY, dup.error)
    assertEquals(7L, dup.offset)
    val bad = assertThrows(YalinParseException::class.java) { YalinJson.parseTree("[1,]") }
    assertEquals(YalinParseError.UNEXPECTED_CHARACTER, bad.error)
    val deep = "[".repeat(1024) + "]".repeat(1024)
    var v = YalinJson.parseTree(deep)
    repeat(1023) { v = (v as YalinArray)[0] }
    assertEquals(emptyList<YalinValue>(), v)
  }

  @Test
  fun byteBuffers() {
    val framed = "xx".encodeToByteArray() + """{"a":"b"}""".encodeToByteArray() + "yy".encodeToByteArray()
    val direct = ByteBuffer.allocateDirect(framed.size).put(framed)
    direct.position(2).limit(framed.size - 2)
    YalinJson.parse(direct).use { assertEquals("b", it.getString("a")) }
    assertEquals(2, direct.position())
    val heap = ByteBuffer.wrap(framed, 2, framed.size - 4)
    YalinJson.parse(heap).use { assertEquals("b", it.getString("a")) }
    assertEquals(2, heap.position())
  }

  @Test
  fun inputIsCopied() {
    val bytes = """{"a":"b"}""".encodeToByteArray()
    YalinJson.parse(bytes).use { doc ->
      bytes.fill(0)
      assertEquals("b", doc.getString("a"))
    }
  }

  @Test
  fun closeIsIdempotentAndQueriesAfterCloseFail() {
    val doc = YalinJson.parse(json)
    doc.close()
    doc.close()
    assertThrows(IllegalStateException::class.java) { doc.getString("user.name") }
  }

  @Test
  fun concurrentQueriesAndClose() {
    val pool = Executors.newFixedThreadPool(8)
    repeat(20) {
      val doc = YalinJson.parse(json)
      val futures =
        (0 until 64).map {
          pool.submit<Boolean> {
            try {
              doc.getString("user.name") == "Ada"
            } catch (_: IllegalStateException) {
              true // lost the race with close(): must fail cleanly, never crash
            }
          }
        }
      doc.close()
      assertTrue(futures.all { it.get() })
    }
    pool.shutdown()
    assertTrue(pool.awaitTermination(10, TimeUnit.SECONDS))
  }

  @Test
  fun manyDocumentsReuseHandles() {
    val docs = (0 until 1000).map { YalinJson.parse("""{"i":$it}""".encodeToByteArray()) }
    docs.forEachIndexed { i, d -> assertEquals(i.toLong(), d.getLong("i")) }
    docs.forEach { it.close() }
    YalinJson.parse(json).use { assertFalse(it.kind("user") == YalinKind.MISSING) }
  }
}
