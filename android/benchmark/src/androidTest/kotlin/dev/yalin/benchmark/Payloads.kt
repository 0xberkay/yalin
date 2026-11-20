package dev.yalin.benchmark

import com.alibaba.fastjson2.annotation.JSONCreator
import com.squareup.moshi.JsonClass
import kotlinx.serialization.Serializable

/** Product-list payloads generated deterministically to a target size. */
class Payload(val label: String, val text: String, val itemCount: Int) {
  val bytes = text.encodeToByteArray()

  val midIndex = itemCount / 2
  val lastIndex = itemCount - 1

  /** The fields every scenario extracts; identical across libraries. */
  val paths = arrayOf("meta.total", "items[0].name", "items[$midIndex].price", "items[$lastIndex].seller.name")

  override fun toString() = label
}

/** The values each scenario must produce; checked by [EquivalenceTest]. */
data class Extracted(val total: Long, val firstName: String, val midPrice: Double, val lastSeller: String)

object Payloads {
  val all: List<Payload> by lazy {
    listOf("1KB" to 1_024, "16KB" to 16_384, "256KB" to 262_144, "1MB" to 1_048_576)
      .map { (label, size) -> generate(label, size) }
  }

  private fun generate(label: String, targetBytes: Int): Payload {
    val items = StringBuilder()
    var count = 0
    // Leave room for the envelope (~60 bytes).
    while (items.length + 60 < targetBytes || count == 0) {
      if (count > 0) items.append(',')
      appendItem(items, count++)
    }
    val json =
      """{"meta":{"page":1,"total":$count,"next":null},"items":[$items]}"""
    return Payload(label, json, count)
  }

  private fun appendItem(sb: StringBuilder, i: Int) {
    val price = (i * 37 % 100_000) / 100.0 + 0.5
    val rating = (i * 7 % 50) / 10.0
    // Non-ASCII, an escaped quote and a \u escape keep string decoding honest.
    sb.append("""{"id":${1_000_000 + i},"sku":"SKU-${i.toString().padStart(6, '0')}",""")
      .append(""""name":"Café $i \"élég\" thé","price":$price,"inStock":${i % 3 != 0},""")
      .append(""""tags":["t${i % 5}","t${i % 7}","discount"],""")
      .append(""""seller":{"id":${i % 97},"name":"Vendeur ${i % 97}","rating":$rating}}""")
  }
}

// Shared data model: Gson/Jackson/fastjson2 (reflection), Moshi (KSP codegen),
// kotlinx.serialization (compiler plugin). @JSONCreator keeps fastjson2 off the
// synthetic kotlinx constructor; names are explicit because Android has no
// constructor parameter names at runtime.

@Serializable
@JsonClass(generateAdapter = true)
data class Catalog @JSONCreator(parameterNames = ["meta", "items"]) constructor(val meta: Meta, val items: List<Item>)

@Serializable
@JsonClass(generateAdapter = true)
data class Meta @JSONCreator(parameterNames = ["page", "total", "next"]) constructor(val page: Int, val total: Long, val next: String?)

@Serializable
@JsonClass(generateAdapter = true)
data class Item @JSONCreator(parameterNames = ["id", "sku", "name", "price", "inStock", "tags", "seller"]) constructor(
  val id: Long,
  val sku: String,
  val name: String,
  val price: Double,
  val inStock: Boolean,
  val tags: List<String>,
  val seller: Seller,
)

@Serializable
@JsonClass(generateAdapter = true)
data class Seller @JSONCreator(parameterNames = ["id", "name", "rating"]) constructor(val id: Long, val name: String, val rating: Double)
