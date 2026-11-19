package dev.yalin.benchmark

import com.alibaba.fastjson2.JSON
import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import com.google.gson.Gson
import com.google.gson.JsonParser
import com.squareup.moshi.JsonReader
import com.squareup.moshi.Moshi
import dev.yalin.YalinJson
import java.io.ByteArrayInputStream
import java.io.InputStreamReader
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import okio.Buffer
import org.json.JSONObject

/**
 * Each scenario starts from the payload's UTF-8 `ByteArray` and ends with the
 * same [Extracted] values. Every library validates the whole input.
 */
enum class Scenario(val run: (Payload) -> Extracted) {
  /** One native call: parse + look up all paths. */
  YALIN_PROJECT({ p ->
    YalinJson.project(p.bytes, *p.paths).let {
      Extracted(it.getLong(0)!!, it.getString(1)!!, it.getDouble(2)!!, it.getString(3)!!)
    }
  }),

  /** Parse to a native document, query once, close. */
  YALIN_DOCUMENT({ p ->
    YalinJson.parse(p.bytes).use { doc ->
      doc.project(*p.paths).let {
        Extracted(it.getLong(0)!!, it.getString(1)!!, it.getDouble(2)!!, it.getString(3)!!)
      }
    }
  }),

  /** Gson JsonElement tree, then navigate. */
  GSON_TREE({ p ->
    val root = JsonParser.parseReader(reader(p.bytes)).asJsonObject
    val items = root.getAsJsonArray("items")
    Extracted(
      root.getAsJsonObject("meta").get("total").asLong,
      items[0].asJsonObject.get("name").asString,
      items[p.midIndex].asJsonObject.get("price").asDouble,
      items[p.lastIndex].asJsonObject.getAsJsonObject("seller").get("name").asString,
    )
  }),

  /** Moshi Map/List tree, then navigate. */
  MOSHI_TREE({ p ->
    @Suppress("UNCHECKED_CAST")
    val root = JsonReader.of(Buffer().write(p.bytes)).readJsonValue() as Map<String, Any?>
    @Suppress("UNCHECKED_CAST")
    val items = root["items"] as List<Map<String, Any?>>
    @Suppress("UNCHECKED_CAST")
    val lastSeller = items[p.lastIndex]["seller"] as Map<String, Any?>
    Extracted(
      ((root["meta"] as Map<*, *>)["total"] as Double).toLong(),
      items[0]["name"] as String,
      items[p.midIndex]["price"] as Double,
      lastSeller["name"] as String,
    )
  }),

  /** Gson reflective binding to the full model. */
  GSON_MODEL({ p -> gson.fromJson(reader(p.bytes), Catalog::class.java).extract(p) }),

  /** Moshi codegen binding to the full model. */
  MOSHI_MODEL({ p -> catalogAdapter.fromJson(Buffer().write(p.bytes))!!.extract(p) }),

  /** kotlinx.serialization JsonElement tree, then navigate. */
  KOTLINX_TREE({ p ->
    val root = Json.parseToJsonElement(p.bytes.decodeToString()).jsonObject
    val items = root.getValue("items").jsonArray
    Extracted(
      root.getValue("meta").jsonObject.getValue("total").jsonPrimitive.long,
      items[0].jsonObject.getValue("name").jsonPrimitive.content,
      items[p.midIndex].jsonObject.getValue("price").jsonPrimitive.double,
      items[p.lastIndex].jsonObject.getValue("seller").jsonObject.getValue("name").jsonPrimitive.content,
    )
  }),

  /** kotlinx.serialization compiler-generated serializer to the full model. */
  KOTLINX_MODEL({ p -> Json.decodeFromString(Catalog.serializer(), p.bytes.decodeToString()).extract(p) }),

  /** Jackson JsonNode tree, then navigate. */
  JACKSON_TREE({ p ->
    val root = jackson.readTree(p.bytes)
    val items = root.get("items")
    Extracted(
      root.get("meta").get("total").asLong(),
      items.get(0).get("name").asText(),
      items.get(p.midIndex).get("price").asDouble(),
      items.get(p.lastIndex).get("seller").get("name").asText(),
    )
  }),

  /** Jackson + jackson-module-kotlin binding to the full model. */
  JACKSON_MODEL({ p -> jackson.readValue(p.bytes, Catalog::class.java).extract(p) }),

  /** fastjson2 JSONObject tree, then navigate. */
  FASTJSON2_TREE({ p ->
    val root = JSON.parseObject(p.bytes)
    val items = root.getJSONArray("items")
    Extracted(
      root.getJSONObject("meta").getLongValue("total"),
      items.getJSONObject(0).getString("name"),
      items.getJSONObject(p.midIndex).getDoubleValue("price"),
      items.getJSONObject(p.lastIndex).getJSONObject("seller").getString("name"),
    )
  }),

  /** fastjson2 reflective binding to the full model. */
  FASTJSON2_MODEL({ p -> JSON.parseObject(p.bytes, Catalog::class.java).extract(p) }),

  /** Android's built-in org.json, then navigate. */
  ORGJSON_TREE({ p ->
    val root = JSONObject(p.bytes.decodeToString())
    val items = root.getJSONArray("items")
    Extracted(
      root.getJSONObject("meta").getLong("total"),
      items.getJSONObject(0).getString("name"),
      items.getJSONObject(p.midIndex).getDouble("price"),
      items.getJSONObject(p.lastIndex).getJSONObject("seller").getString("name"),
    )
  }),
}

private val gson = Gson()
private val catalogAdapter = Moshi.Builder().build().adapter(Catalog::class.java)
private val jackson = jacksonObjectMapper()

private fun reader(bytes: ByteArray) = InputStreamReader(ByteArrayInputStream(bytes), Charsets.UTF_8)

private fun Catalog.extract(p: Payload) =
  Extracted(meta.total, items[0].name, items[p.midIndex].price, items[p.lastIndex].seller.name)
