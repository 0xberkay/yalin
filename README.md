# Yalın

Yalın validates UTF-8 JSON and reads selected fields on Android. The Rust parser builds a native index and decodes values when queried. Kotlin and Java callers can read several fields in one JNI call.

## Usage

Yalın supports Android API 24 and later on `arm64-v8a` and `x86_64`.

```kotlin
import dev.yalin.YalinJson

val input = """{"user":{"name":"Ada"},"items":[{"price":12.5}]}""".encodeToByteArray()
val fields = YalinJson.project(input, "user.name", "items[0].price")
val name = fields.getString(0)
val price = fields.getDouble(1)
```

Use `project` for a single batch of lookups. To query the same input repeatedly, parse once and close the document with `use`.

```kotlin
YalinJson.parse(input).use { document ->
    println(document.getString("user.name"))
}
```

Paths support dotted keys and array indices. Typed getters return `null` for a missing field or a JSON `null`. Use `kind` to distinguish them. Invalid JSON throws `YalinParseException`.

## Build and test

The source build uses a Rust toolchain with the Android targets, JDK 21 to run Gradle, JDK 17 as the Kotlin toolchain, Android SDK Platform 37 and NDK `28.2.13676358`.

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo test --workspace --locked
cd android
./gradlew :yalin-android:testDebugUnitTest :yalin-android:assembleRelease
```

See the [Android benchmarks](docs/benchmarks.md) for device measurements and the [Rust benchmarks](docs/core-benchmarks.md) for parser-only measurements. The [original design notes](docs/design-notes.md) record the initial plan.

## License

Yalın is available under the [MIT License](LICENSE). Sample app files with Android Open Source Project notices retain their [Apache 2.0 license](licenses/Apache-2.0.txt).
