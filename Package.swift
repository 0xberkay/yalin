// swift-tools-version:5.9
// Swift package for Yalın on iOS and macOS. Sources live in apple/.
//
// Apple platforms link apple/YalinFFI.xcframework: build it first with
// apple/build-xcframework.sh. Linux (CI and local tests) links
// target/release/libyalin_ffi.a: build it with `cargo build --release -p yalin-ffi`.
import PackageDescription

#if os(Linux)
let ffi: Target = .systemLibrary(name: "YalinFFI", path: "apple/Sources/YalinFFI")
let linkerSettings: [LinkerSetting] = [
  .unsafeFlags(["-L\(Context.packageDirectory)/target/release"]),
  // Needed by the Rust standard library in a static library.
  .linkedLibrary("m"),
  .linkedLibrary("dl"),
  .linkedLibrary("pthread"),
]
#else
let ffi: Target = .binaryTarget(name: "YalinFFI", path: "apple/YalinFFI.xcframework")
let linkerSettings: [LinkerSetting] = []
#endif

let package = Package(
  name: "Yalin",
  platforms: [.iOS(.v13), .macOS(.v10_15)],
  products: [
    .library(name: "Yalin", targets: ["Yalin"]),
  ],
  targets: [
    ffi,
    .target(
      name: "Yalin",
      dependencies: ["YalinFFI"],
      path: "apple/Sources/Yalin",
      linkerSettings: linkerSettings
    ),
    .executableTarget(
      name: "yalin-benchmark",
      dependencies: ["Yalin"],
      path: "apple/Benchmarks/YalinBenchmark"
    ),
    .testTarget(
      name: "YalinTests",
      dependencies: ["Yalin"],
      path: "apple/Tests/YalinTests"
    ),
  ]
)
