# Yalın for iOS and macOS (Swift)

The Swift package wraps the same Rust core as the Android library. `crates/yalin-ffi` exposes a small C ABI and is built as a static library inside `apple/YalinFFI.xcframework`; the Swift sources are in `apple/Sources/Yalin`.

Requires iOS 13 or macOS 10.15 and Swift 5.9 (Xcode 15) or later. The build produces slices for iOS devices (`arm64`), the iOS simulator (`arm64`, `x86_64`) and macOS (`arm64`, `x86_64`).

## Build and add to an app

On a Mac with Xcode and rustup:

```sh
apple/build-xcframework.sh   # builds apple/YalinFFI.xcframework
swift test                   # runs the Swift tests on macOS
```

Then add the repository folder to your Xcode project as a local package (File › Add Package Dependencies › Add Local…) and link the `Yalin` product. The package manifest is `Package.swift` at the repository root.

To publish a binary release, zip the XCFramework, attach it to a GitHub release and, in `Package.swift`, replace the `path:` binary target with `url:` and the `checksum:` printed by `swift package compute-checksum YalinFFI.xcframework.zip`. Apps can then depend on the repository by URL without Rust installed.

## Usage

```swift
import Yalin

let data = Data(#"{"user":{"name":"Ada"},"items":[{"price":12.5}]}"#.utf8)

// A few fields, one native call, no tree.
let fields = try Yalin.project(data, "user.name", "items[0].price")
let name = try fields.string(at: 0)    // "Ada"
let price = try fields.double(at: 1)   // 12.5

// Many queries over one document. Freed when the last reference goes away.
let doc = try Yalin.parse(data)
let same = try doc.string("user.name")

// The whole tree as Swift values.
let tree = try Yalin.parseTree(data)
let first = tree["items"]?[0]?["price"]?.doubleValue   // 12.5
if case let .object(root) = tree {
  for (key, value) in root { print(key, value.kind) }
}
```

Inputs are any `ContiguousBytes` (`Data`, `[UInt8]`, …) or a `String`. Every call validates the whole input first.

- **Missing vs null.** Typed getters return `nil` for a missing path and for JSON `null`; `kind(at:)` / `kind(_:)` tells them apart (`.missing` or `.null`).
- **Errors.** Everything throws `YalinError`: `.parse(kind, offset:)` for invalid JSON (byte offset), `.invalidPath`, `.typeMismatch(path:expected:found:)`, `.notRepresentable` (for example `int64` on `1.5` or on `1e400`).
- **Numbers.** In trees an integer that fits `Int64` is `.int`, a number with a fraction or exponent is `.double` unless it overflows, and anything else is `.bigNumber(text)` (`decimalValue` parses it). In projections an integer outside `Int64` that fits a `Double` reads with `double(at:)`.
- **Duplicate keys.** `parseTree` rejects them (`.duplicateKey`); path queries take the first occurrence.
- **Threads.** `YalinDocument`, `YalinProjection` and `YalinValue` are immutable and `Sendable`.

## Why it is fast in Swift

- **One crossing per call.** `project`, `document.project` and `parseTree` each make a single C call, however many paths or values are involved.
- **No input copy for one-shot calls.** `project` and `parseTree` parse the caller's bytes in place through `withUnsafeBytes`; only `parse` copies them, because the document outlives the call.
- **No output copy.** Results are read straight from the buffer Rust allocated; it is freed when the projection is released, or right after the tree is built.
- **Lazy projections.** A projection creates a `String` only for the fields you read.
- **Fewer string decodes in trees.** The encoder sends repeated keys and short strings as slot references, so the Swift decoder creates each of them once per tree. Strings of up to 15 UTF-8 bytes are stored inline by Swift, without a heap allocation.
- **Small objects stay cheap.** A `YalinObject` is one class instance with one member array. Objects with more than 16 members build a hash index on first lookup; smaller ones scan.

## Benchmarks

`swift run -c release yalin-benchmark` times the Android benchmark's product catalog against `JSONSerialization` and `JSONDecoder`. On a Linux x86_64 container (Swift 6.2, swift-corelibs-foundation), medians:

| Input | `project`, 4 fields | `parseTree` | `JSONSerialization` | `JSONDecoder` |
|---|---:|---:|---:|---:|
| 1 KB | 0.003 ms | 0.013 ms | 0.071 ms | 0.064 ms |
| 16 KB | 0.033 ms | 0.136 ms | 0.684 ms | 0.959 ms |
| 256 KB | 0.535 ms | 2.24 ms | 12.06 ms | 15.54 ms |
| 1 MB | 2.31 ms | 8.51 ms | 69.05 ms | 58.89 ms |

Apple's Foundation is faster than the Linux one, so expect smaller ratios on iOS. Device numbers have not been measured yet; to get them, run the same code from a Release build of an app on an iPhone.

## Safety rules at the boundary

- Input pointers are read only during the call and never retained.
- A document is a Rust `Box` owned by one Swift `final class`; its pointer is private and freed in `deinit`, so it cannot be used after it is freed.
- Each result buffer has one Swift owner that frees it exactly once.
- Rust panics are caught at the boundary and surface as `YalinError.internal`, never as a crash across the C ABI.
