package dev.yalin.benchmark

import com.alibaba.fastjson2.JSON
import com.alibaba.fastjson2.JSONArray
import com.alibaba.fastjson2.JSONObject
import com.fasterxml.jackson.databind.JsonNode
import com.google.gson.JsonElement
import com.google.gson.JsonParser
import com.squareup.moshi.JsonReader
import dev.yalin.YalinArray
import dev.yalin.YalinBigNumber
import dev.yalin.YalinBoolean
import dev.yalin.YalinDouble
import dev.yalin.YalinJson
import dev.yalin.YalinLong
import dev.yalin.YalinNull
import dev.yalin.YalinObject
import dev.yalin.YalinString
import dev.yalin.YalinValue
import java.io.ByteArrayInputStream
import java.io.InputStreamReader
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import okio.Buffer

/**
 * Each scenario builds and returns the library's full JSON tree. `*_BYTES`
 * scenarios start from the payload's UTF-8 `ByteArray` (decoding included where
 * the library takes text); `*_STRING` scenarios start from a `String`.
 * [canonical] maps every tree to the same plain form for [EquivalenceTest].
 */
enum class TreeScenario(val run: (Payload) -> Any, val canonical: (Any) -> Any?) {
  YALIN_BYTES({ YalinJson.parseTree(it.bytes) }, { yalin(it as YalinValue) }),
  GSON_BYTES(
    { JsonParser.parseReader(InputStreamReader(ByteArrayInputStream(it.bytes), Charsets.UTF_8)) },
    { gson(it as JsonElement) },
  ),
  MOSHI_BYTES({ JsonReader.of(Buffer().write(it.bytes)).readJsonValue()!! }, { plain(it) }),
  KOTLINX_BYTES({ Json.parseToJsonElement(it.bytes.decodeToString()) }, { kotlinx(it as kotlinx.serialization.json.JsonElement) }),
  JACKSON_BYTES({ jackson.readTree(it.bytes) }, { jackson(it as JsonNode) }),
  FASTJSON2_BYTES({ JSON.parseObject(it.bytes) }, { plain(it) }),
  YALIN_STRING({ YalinJson.parseTree(it.text) }, { yalin(it as YalinValue) }),
  KOTLINX_STRING({ Json.parseToJsonElement(it.text) }, { kotlinx(it as kotlinx.serialization.json.JsonElement) }),
  FASTJSON2_STRING({ JSON.parseObject(it.text) }, { plain(it) }),
}

private val jackson = com.fasterxml.jackson.databind.ObjectMapper()

// Canonical form: Map (insertion order ignored by equals), List, String, Boolean,
// null, and every number as Double (all payload numbers are exact in Double).

private fun yalin(v: YalinValue): Any? =
  when (v) {
    YalinNull -> null
    is YalinBoolean -> v.value
    is YalinLong -> v.value.toDouble()
    is YalinDouble -> v.value
    is YalinBigNumber -> v.value.toDouble()
    is YalinString -> v.value
    is YalinArray -> v.map(::yalin)
    is YalinObject -> v.mapValues { yalin(it.value) }
  }

private fun gson(v: JsonElement): Any? =
  when {
    v.isJsonNull -> null
    v.isJsonObject -> v.asJsonObject.entrySet().associate { it.key to gson(it.value) }
    v.isJsonArray -> v.asJsonArray.map(::gson)
    v.asJsonPrimitive.isNumber -> v.asDouble
    v.asJsonPrimitive.isBoolean -> v.asBoolean
    else -> v.asString
  }

private fun kotlinx(v: kotlinx.serialization.json.JsonElement): Any? =
  when (v) {
    JsonNull -> null
    is JsonObject -> v.mapValues { kotlinx(it.value) }
    is JsonArray -> v.map(::kotlinx)
    is JsonPrimitive ->
      when {
        v.isString -> v.content
        v.content == "true" || v.content == "false" -> v.boolean
        else -> v.double
      }
  }

private fun jackson(v: JsonNode): Any? =
  when {
    v.isNull -> null
    v.isObject -> v.properties().associate { it.key to jackson(it.value) }
    v.isArray -> v.map(::jackson)
    v.isNumber -> v.asDouble()
    v.isBoolean -> v.asBoolean()
    else -> v.asText()
  }

/** Moshi and fastjson2 trees are already `Map`/`List`/boxed values. */
private fun plain(v: Any?): Any? =
  when (v) {
    is JSONObject -> v.entries.associate { it.key to plain(it.value) }
    is JSONArray -> v.map(::plain)
    is Map<*, *> -> v.entries.associate { it.key to plain(it.value) }
    is List<*> -> v.map(::plain)
    is Number -> v.toDouble()
    else -> v
  }
