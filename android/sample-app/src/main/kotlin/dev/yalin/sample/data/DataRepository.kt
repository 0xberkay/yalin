package dev.yalin.sample.data

import dev.yalin.YalinJson
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

interface DataRepository {
  val data: Flow<List<String>>
}

/** Reads a few fields from a sample API response with Yalın. */
class DefaultDataRepository : DataRepository {
  override val data: Flow<List<String>> = flow {
    val paths = arrayOf("user.name", "user.age", "user.email", "items[1].price")
    val fields = YalinJson.project(SAMPLE, *paths)
    emit(
      paths.mapIndexed { i, path ->
        val value =
          when (fields.kind(i)) {
            dev.yalin.YalinKind.STRING -> fields.getString(i)
            dev.yalin.YalinKind.NUMBER -> fields.getDouble(i).toString()
            else -> fields.kind(i).name.lowercase()
          }
        "$path = $value"
      }
    )
  }

  private companion object {
    val SAMPLE =
      """{"user":{"name":"Ada","age":36,"email":null},"items":[{"price":9.99},{"price":19.5}]}"""
        .encodeToByteArray()
  }
}
