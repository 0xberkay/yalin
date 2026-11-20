package dev.yalin.benchmark

import com.alibaba.fastjson2.annotation.JSONCreator
import com.squareup.moshi.JsonClass
import dev.yalin.benchmarkinput.BenchmarkPayloadSize
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
    BenchmarkPayloadSize.entries.map { size ->
      val input = size.generate()
      Payload(size.label.replace(" ", ""), input.text, input.itemCount)
    }
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
