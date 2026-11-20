package dev.yalin.benchmarkinput

data class BenchmarkInput(val text: String, val itemCount: Int)

/** Deterministic product catalog used by both the app and the Android benchmark. */
enum class BenchmarkPayloadSize(val label: String, private val targetBytes: Int) {
  SMALL("1 KB", 1_024),
  MEDIUM("16 KB", 16_384),
  LARGE("256 KB", 262_144),
  EXTRA_LARGE("1 MB", 1_048_576);

  fun generate(): BenchmarkInput {
    val items = StringBuilder()
    var count = 0
    // Leave room for the envelope (~60 bytes).
    while (items.length + 60 < targetBytes || count == 0) {
      if (count > 0) items.append(',')
      appendItem(items, count++)
    }
    val json =
      """{"meta":{"page":1,"total":$count,"next":null},"items":[$items]}"""
    return BenchmarkInput(json, count)
  }

  private fun appendItem(sb: StringBuilder, i: Int) {
    val price = (i * 37 % 100_000) / 100.0 + 0.5
    val rating = (i * 7 % 50) / 10.0
    // Non-ASCII text and escaped quotes exercise string decoding.
    sb.append("""{"id":${1_000_000 + i},"sku":"SKU-${i.toString().padStart(6, '0')}",""")
      .append(""""name":"Café $i \"élég\" thé","price":$price,"inStock":${i % 3 != 0},""")
      .append(""""tags":["t${i % 5}","t${i % 7}","discount"],""")
      .append(""""seller":{"id":${i % 97},"name":"Vendeur ${i % 97}","rating":$rating}}""")
  }
}
