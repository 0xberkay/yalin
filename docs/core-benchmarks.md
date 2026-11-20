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

## Samsung Galaxy A30s (Exynos 7904), Android 11, 2026-11-20

Current core at commit `17dce04`, running on cpu7 (2.08 GHz maximum). Clocks were not
locked; each result is the median of 11 runs of 200 ms after a warm-up. All scenarios
ran in one invocation, using the same generated payloads. These are native-only results;
they exclude JNI and Kotlin object creation.

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---:|---:|---:|---:|
| utf8_only | 1.06 µs | 14.52 µs | 228.75 µs | 0.918 ms |
| yalin_validate | 12.11 µs | 162.84 µs | 2.855 ms | 11.390 ms |
| yalin_extract | 14.02 µs | 166.15 µs | 2.901 ms | 11.725 ms |
| yalin_tree | 22.57 µs | 306.09 µs | 5.347 ms | 21.341 ms |
| serde_ignored | 6.61 µs | 91.54 µs | 1.435 ms | 5.738 ms |
| serde_value | 46.08 µs | 827.61 µs | 16.082 ms | 62.294 ms |
| sonic_ignored | 7.94 µs | 111.82 µs | 1.758 ms | 7.048 ms |
| sonic_value | 8.01 µs | 93.45 µs | 1.974 ms | 7.892 ms |

For the 1 MB four-field workload, `sonic_value` took 7.892 ms versus 11.725 ms
for `yalin_extract`; `serde_value` took 62.294 ms. All three return the same
four values, but they use different intermediate representations and may differ in
validation behavior. `yalin_tree` additionally encodes a tree for transfer to Kotlin,
so it is not a direct full-tree comparison with the Rust `Value` rows. The payload is
one synthetic catalog; results can differ on other data and devices.

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
