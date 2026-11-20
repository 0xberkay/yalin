package dev.yalin.sample.data

import android.os.Build
import com.alibaba.fastjson2.JSON
import com.google.gson.JsonParser
import com.squareup.moshi.JsonReader
import dev.yalin.YalinArray
import dev.yalin.YalinJson
import dev.yalin.YalinObject
import dev.yalin.YalinString
import dev.yalin.YalinValue
import dev.yalin.benchmarkinput.BenchmarkPayloadSize
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import okio.Buffer

enum class Dataset(val catalogSize: BenchmarkPayloadSize? = null) {
  CATALOG_1KB(BenchmarkPayloadSize.SMALL),
  CATALOG_16KB(BenchmarkPayloadSize.MEDIUM),
  CATALOG_256KB(BenchmarkPayloadSize.LARGE),
  CATALOG_1MB(BenchmarkPayloadSize.EXTRA_LARGE),
  VEGA_CARS,
}

data class ParserTime(val parser: String, val medianNanos: Long)

data class ParseResult(
  val byteCount: Int,
  val recordCount: Int,
  val times: List<ParserTime>,
  val cars: List<String> = emptyList(),
)

interface DataRepository {
  suspend fun measure(dataset: Dataset): ParseResult
}

const val MEASURED_RUNS = 5

class DefaultDataRepository : DataRepository {
  override suspend fun measure(dataset: Dataset): ParseResult {
    val downloaded = if (dataset == Dataset.VEGA_CARS) withContext(Dispatchers.IO) { downloadCars() } else null

    return withContext(Dispatchers.Default) {
      val bytes = downloaded ?: checkNotNull(dataset.catalogSize).generate().text.encodeToByteArray()
      val parsers = TreeParser.entries.filter {
        it != TreeParser.FASTJSON2 || Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
      }
      val (recordCount, cars) = readSummary(YalinJson.parseTree(bytes), dataset)
      parsers.filter { it != TreeParser.YALIN }.forEach { parser ->
        ensureActive()
        checkNotNull(parser.parse(bytes))
      }

      val samples = Array(parsers.size) { LongArray(MEASURED_RUNS) }
      repeat(MEASURED_RUNS) { round ->
        // Shift which parser runs first on each pass.
        parsers.indices.forEach { offset ->
          ensureActive()
          val index = (offset + round) % parsers.size
          val parse = parsers[index].parse
          val start = System.nanoTime()
          val parsed = parse(bytes)
          val elapsed = System.nanoTime() - start
          checkNotNull(parsed)
          samples[index][round] = elapsed
        }
      }
      val times = parsers.mapIndexed { index, parser ->
        ParserTime(parser.label, samples[index].sorted()[MEASURED_RUNS / 2])
      }.sortedBy { it.medianNanos }
      ParseResult(bytes.size, recordCount, times, cars)
    }
  }

  private fun readSummary(tree: YalinValue, dataset: Dataset): Pair<Int, List<String>> {
    if (dataset != Dataset.VEGA_CARS) {
      val items = (tree as YalinObject)["items"] as YalinArray
      return items.size to emptyList()
    }
    val cars = (tree as YalinArray).map { record ->
      val car = record as YalinObject
      val name = (car["Name"] as YalinString).value
      val origin = (car["Origin"] as YalinString).value
      "$name · $origin"
    }
    return cars.size to cars
  }

  private fun downloadCars(): ByteArray {
    val connection = (URL(CARS_URL).openConnection() as HttpURLConnection).apply {
      connectTimeout = 10_000
      readTimeout = 10_000
    }
    try {
      val status = connection.responseCode
      if (status != HttpURLConnection.HTTP_OK) throw IOException("HTTP $status")
      return connection.inputStream.use { it.readBytes() }
    } finally {
      connection.disconnect()
    }
  }

  private companion object {
    const val CARS_URL = "https://raw.githubusercontent.com/vega/vega-datasets/" +
      "192178cf90a83fedccbab80c1f067c29cac6a697/data/cars.json"
  }
}

private enum class TreeParser(val label: String, val parse: (ByteArray) -> Any?) {
  YALIN("Yalın", { YalinJson.parseTree(it) }),
  FASTJSON2("fastjson2", { JSON.parse(it) }),
  GSON("Gson", { JsonParser.parseReader(it.inputStream().reader(Charsets.UTF_8)) }),
  KOTLINX("kotlinx.serialization", { Json.parseToJsonElement(it.decodeToString()) }),
  MOSHI("Moshi", { JsonReader.of(Buffer().write(it)).readJsonValue() }),
}
