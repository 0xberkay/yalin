# Core benchmarks (no JNI)

Crate: `crates/yalin-bench`, a plain binary that runs on the host or on a device via `adb`.
It uses the same payloads as `android/benchmark` (a byte-for-byte port of `Payloads.kt`) and
checks that yalin and sonic-rs extract the same 4 values as serde_json before timing anything.

| Scenario | Work |
|---|---|
| `utf8_only` | `std::str::from_utf8`: lower bound for a single pass over the bytes |
| `yalin_validate` | `yalin_core::validate` (both stages, tape built and dropped) |
| `yalin_extract` | `Document::parse` + 4 `get`s (what `YalinJson.project` does in native code) |
| `yalin_tree` | parse, walk and encode the whole tree (what `YalinJson.parseTree` does in native code) |
| `serde_ignored` | `serde_json::from_slice::<IgnoredAny>`: syntax check, no tree |
| `serde_value` | `serde_json::Value` tree + navigation |
| `sonic_ignored` | `sonic_rs::from_slice::<IgnoredAny>` |
| `sonic_value` | `sonic_rs::Value` tree (arena) + navigation |

The `*_ignored` rows are not the same work as yalin: they build no index. serde_json may also skip
some string checks when it ignores a value, so treat them as a rough upper bound.

## Run

```bash
NDK=~/Android/Sdk/ndk/28.2.13676358
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang
cargo build -p yalin-bench --profile profiling --target aarch64-linux-android
adb push target/aarch64-linux-android/profiling/yalin-bench /data/local/tmp/
adb shell taskset 80 /data/local/tmp/yalin-bench          # pin to cpu7; --runs N --ms MS --filter yalin
```

`profiling` is the release profile (fat LTO, codegen-units=1) with symbols kept.
To profile: `yalin-bench --spin <scenario> <size> <seconds>` runs a hot loop; record it with the
NDK's `simpleperf record --call-graph fp` (it works without root from `/data/local/tmp`).

## Infinix X6833B (Helio G99), Android 13, 2026-11-19

Clocks not locked. Each cell is the median of 11 runs of 200 ms each; MB = 2^20 bytes.

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB | MB/s at 1 MB |
|---|---|---|---|---|---|
| **A76 (cpu7)** | | | | | |
| utf8_only | 0.60 µs | 8.46 µs | 0.139 ms | 0.551 ms | 1816 |
| yalin_validate | 8.23 µs | 127 µs | 2.73 ms | 11.15 ms | 90 |
| yalin_extract | 9.29 µs | 130 µs | 2.84 ms | 11.81 ms | 85 |
| serde_ignored | 2.96 µs | 40.5 µs | 0.651 ms | 2.59 ms | 386 |
| serde_value | 20.3 µs | 297 µs | 5.80 ms | 24.44 ms | 41 |
| sonic_ignored | 3.59 µs | 49.8 µs | 0.781 ms | 3.11 ms | 322 |
| sonic_value | 3.72 µs | 41.2 µs | 1.20 ms | 4.93 ms | 203 |
| **A55 (cpu0)** | | | | | |
| yalin_extract | 28.0 µs | 394 µs | 7.52 ms | 30.85 ms | 32 |
| serde_ignored | 8.29 µs | 114 µs | 1.80 ms | 7.25 ms | 138 |
| sonic_value | 11.8 µs | 133 µs | 3.33 ms | 14.88 ms | 67 |

p10–p90 spread is under 1% on the A76 and under 4% on the A55.

Host (x86_64, 1 MB): yalin_extract 506 MB/s, serde_ignored 1408, sonic_value 1556, utf8_only 7248.

### `yalin_tree`, A76 (cpu7), 2026-11-20

Median of 5 runs of 200 ms each, measured after the tree encoder optimizations.

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB | MB/s at 1 MB |
|---|---|---|---|---|---|
| yalin_validate | 5.04 µs | 67.9 µs | 1.25 ms | 5.26 ms | 190 |
| yalin_tree | 9.83 µs | 127 µs | 2.34 ms | 10.10 ms | 99 |

Encoding the tree roughly doubles the native time. Its profile is the walk itself
(reading the tape, the container stack and the duplicate-key check, ~19%) and the
key/string slot cache (hashing and comparing, ~17% with `memcmp`). The validation
rows are faster than in the table above because the parser has since been optimized.
