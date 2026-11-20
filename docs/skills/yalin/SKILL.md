---
name: yalin
description: Parse and read JSON on Android with Yalın (io.github.0xberkay:yalin-android, package dev.yalin), a strict, fast JSON library backed by a native Rust parser. Use when adding Yalın to an Android project, replacing Gson/Moshi/kotlinx.serialization/Jackson/fastjson2/org.json for reading JSON, pulling a few fields out of a large response, building a JSON tree in Kotlin or Java, or handling YalinParseException, YalinTypeException or YalinPathException.
license: Apache-2.0
---

# Yalın

Yalın reads UTF-8 JSON on Android. It validates the whole input in native code, then returns
either selected fields or an immutable tree. It **reads** JSON only: it does not write JSON and
does not bind to data classes. Map values to your own types by hand.

## Setup

```kotlin
// app/build.gradle.kts
dependencies {
    implementation("io.github.0xberkay:yalin-android:0.0.2")
}
```

Check Maven Central for a newer version before pinning. Requirements:

- `minSdk` 24 or higher, `compileSdk` 37 or higher.
- Native code ships for `arm64-v8a` and `x86_64` only. If the app also targets `armeabi-v7a`
  or `x86`, Yalın throws `UnsatisfiedLinkError` on those devices. Either restrict the app with
  `ndk { abiFilters += listOf("arm64-v8a", "x86_64") }` or keep another parser as a fallback.
- The AAR ships its own R8 rules. No annotations, KSP or reflection are needed.

## Pick an entry point

All entry points live on `dev.yalin.YalinJson`. Prefer a `ByteArray` input (for example
OkHttp's `response.body.bytes()`); a `String` must be encoded to UTF-8 first, which costs time.

| Need | Use | Returns |
|---|---|---|
| A few fields, once | `YalinJson.project(bytes, "a.b", "c[0]")` | `YalinProjection` (indexed by path order) |
| Many lookups on the same input | `YalinJson.parse(bytes).use { doc -> ... }` | `YalinDocument` (must be closed) |
| The whole document | `YalinJson.parseTree(bytes)` | `YalinValue` tree |

Rule of thumb: when the code reads a handful of fields from a big response, use `project`. It
is several times faster than any tree. Use `parseTree` when the code walks most of the data.

### `project`: several fields in one call

```kotlin
import dev.yalin.YalinJson

val fields = YalinJson.project(bytes, "meta.total", "items[0].name", "items[0].price")
val total: Long? = fields.getLong(0)
val name: String? = fields.getString(1)
val price: Double? = fields.getDouble(2)
```

### `parse`: reusable document

```kotlin
YalinJson.parse(bytes).use { doc ->
    val count = doc.getLong("meta.total") ?: 0
    val page = doc.project("items[0].id", "items[1].id")
}
```

A `YalinDocument` holds native memory until `close()`. Always use `use {}` or `try/finally`;
an unclosed document is never freed by the GC. Queries are thread-safe, and a query after
`close()` throws `IllegalStateException`. `parse` also accepts a `ByteBuffer` (direct buffers
are read without an extra copy).

### `parseTree`: immutable tree

```kotlin
import dev.yalin.*

val root = YalinJson.parseTree(bytes) as YalinObject      // Map<String, YalinValue>
val items = root["items"] as YalinArray                   // List<YalinValue>
val names = items.map { ((it as YalinObject)["name"] as YalinString).value }
```

`YalinValue` is a sealed interface:

| JSON | Type | Value |
|---|---|---|
| `null` | `YalinNull` (object) | |
| `true` / `false` | `YalinBoolean` | `.value: Boolean` |
| string | `YalinString` | `.value: String` |
| integer fitting `Long` | `YalinLong` | `.value: Long` |
| number with `.`, `e` or `E` that fits `Double` | `YalinDouble` | `.value: Double` |
| larger integer, or a number that overflows `Double` | `YalinBigNumber` | `.value: BigDecimal` |
| array | `YalinArray` | `List<YalinValue>` |
| object | `YalinObject` | `Map<String, YalinValue>`, keys in document order |

Use `when (value) { is YalinString -> ...; is YalinLong -> ... }` for exhaustive handling.

## Paths

- `user.name`: object member. `items[3].price`: zero-based array index. `[0].id`: index into a
  top-level array. `matrix[1][2]`: nested arrays. `""`: the root value.
- Keys are matched after unescaping. A key containing `.`, `[` or `]`, or an empty key, cannot be
  addressed by a path; use `parseTree` for those.
- With a duplicate key, paths return the first occurrence. `parseTree` rejects duplicates.

## Missing, null and wrong types

- Typed getters (`getString`, `getLong`, `getDouble`, `getBoolean`) return `null` for both a
  missing path and a JSON `null`. Call `kind(index)` or `doc.kind(path)` to tell them apart:
  `YalinKind.MISSING` versus `YalinKind.NULL`.
- A value of another type throws `YalinTypeException(path, expected, found)`. This includes
  indexing into an object or reading a member of an array on the way.
- `getLong` throws `ArithmeticException` for a fractional or out-of-range number. `getDouble`
  accepts integers and throws `ArithmeticException` only for numbers that overflow `Double`.
- A malformed path throws `YalinPathException(path, position)`.

## Errors

Invalid JSON throws `YalinParseException` with `error: YalinParseError` and `offset` (UTF-8
byte offset). Yalın is strict (RFC 8259): it rejects trailing commas, comments, single quotes,
unquoted keys, `NaN`, invalid UTF-8, control characters in strings, bad escapes, lone
surrogates and trailing data. Input that Gson or fastjson2 accepted leniently may fail here.
That is expected; fix the producer or catch the exception.

```kotlin
try {
    YalinJson.project(bytes, "id")
} catch (e: YalinParseException) {
    log("bad JSON: ${e.error} at byte ${e.offset}")
}
```

## Java

```java
YalinProjection fields = YalinJson.project(bytes, "user.name", "items[0].price");
String name = fields.getString(0);
Double price = fields.getDouble(1);

try (YalinDocument doc = YalinJson.parse(bytes)) {
    Long total = doc.getLong("meta.total");
}
```

## Migrating from another parser

1. Find where JSON is read and which fields the code actually uses.
2. If only some fields are used, replace the model or tree parse with one `project` call that
   lists those paths, and build the result object from the getters.
3. If the whole structure is used, call `parseTree` and convert `YalinObject`/`YalinArray` into
   the app's types with small mapping functions.
4. Keep the old library for writing JSON; Yalın has no serializer.
5. Add a test with a real payload that compares the old and new results.
