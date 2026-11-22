# Android benchmarks

Module: `android/benchmark` (AndroidX Microbenchmark 1.5.0, release / non-debuggable).

```bash
cd android && ./gradlew :benchmark:connectedReleaseAndroidTest
```

Raw output: `android/benchmark/build/outputs/connected_android_test_additional_output/releaseAndroidTest/connected/<device>/dev.yalin.benchmark.test-benchmarkData.json`.

## Workload

Deterministic product list (`Payloads.kt`) at 1 KB, 16 KB, 256 KB, 1 MB. Every
scenario starts from the UTF-8 `ByteArray`, validates the whole input and returns
the same 4 values (`meta.total`, `items[0].name`, `items[mid].price`,
`items[last].seller.name`). `EquivalenceTest` asserts all scenarios agree.

| Scenario | Work |
|---|---|
| `YALIN_PROJECT` | `YalinJson.project(bytes, paths)`: one native call |
| `YALIN_DOCUMENT` | `YalinJson.parse` → `project` → `close` |
| `GSON_TREE` | `JsonParser.parseReader` → navigate `JsonElement` |
| `MOSHI_TREE` | `JsonReader.readJsonValue` → navigate `Map`/`List` |
| `GSON_MODEL` | `Gson.fromJson` to full model (reflection) |
| `MOSHI_MODEL` | Moshi KSP codegen adapter to full model |
| `KOTLINX_TREE` | `Json.parseToJsonElement` → navigate |
| `KOTLINX_MODEL` | kotlinx.serialization compiler-plugin serializer to full model |
| `JACKSON_TREE` | `ObjectMapper.readTree` → navigate `JsonNode` |
| `JACKSON_MODEL` | Jackson + jackson-module-kotlin to full model |
| `FASTJSON2_TREE` | `JSON.parseObject` → navigate `JSONObject` |
| `FASTJSON2_MODEL` | fastjson2 reflective binding to full model |
| `ORGJSON_TREE` | Android built-in `org.json.JSONObject` → navigate |

Caveats:
- `*_MODEL` materializes every object; yalin and `*_TREE` only extract 4 fields
  (but still validate everything). Treat model rows as "typical app usage", not equivalent work.
- Gson, fastjson2 and org.json parse leniently by default; Moshi, kotlinx.serialization,
  Jackson and yalin are strict.
- kotlinx and org.json scenarios include `ByteArray` → `String` decoding (their usual input).
- Libraries: Gson 2.15.0, Moshi 1.15.2, kotlinx.serialization 1.13.0, Jackson 2.23.0,
  fastjson2 2.0.66.android8. Benchmark module minSdk 26 (Jackson).

## Full tree (`TreeBenchmark`)

Every scenario builds and returns the library's whole JSON tree, with every value
materialized. `EquivalenceTest.allTreesAgree` maps each tree to the same plain
`Map`/`List`/`String`/`Double` form and checks that all of them match before the timings count.

| Scenario | Work |
|---|---|
| `YALIN_BYTES` | `YalinJson.parseTree(ByteArray)`: one native call, then decode into `YalinValue` |
| `YALIN_STRING` | `YalinJson.parseTree(String)`: includes the UTF-8 encoding |
| `GSON_BYTES` | `JsonParser.parseReader` |
| `MOSHI_BYTES` | `JsonReader.readJsonValue` |
| `KOTLINX_BYTES` / `_STRING` | `Json.parseToJsonElement`; `_BYTES` includes `decodeToString` |
| `JACKSON_BYTES` | `ObjectMapper.readTree` |
| `FASTJSON2_BYTES` / `_STRING` | `JSON.parseObject` |

Yalın rejects duplicate keys in a tree. Fastjson2 and Gson keep the last one.

### Infinix X6833B, Android 13, 2026-11-23

Release build at `a1f5740`, AOT-compiled with the `speed` filter. The phone was
charging and the CPU clocks were unlocked. Both equivalence tests and all 52
four-field benchmark cases passed. The ADB connection dropped after 28 full-tree
cases returned results. Missing cases are marked with a dash. The archived
2026-11-20 run below contains the complete 1 MB full-tree comparison.

Median time (ART allocations per operation):

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---|---|---|---|
| **YALIN_BYTES** | 24.27 µs (77) | 260.85 µs (1,045) | 3.99 ms (17,619) | 16.00 ms (75,455) |
| GSON_BYTES | 71.83 µs (401) | 727.22 µs (5,549) | 10.99 ms (86,533) | — |
| MOSHI_BYTES | 69.39 µs (329) | 1.02 ms (4,823) | 16.25 ms (75,579) | — |
| KOTLINX_BYTES | 59.06 µs (340) | 820.55 µs (4,834) | 12.33 ms (75,578) | — |
| JACKSON_BYTES | 44.99 µs (267) | 620.54 µs (3,875) | 9.53 ms (60,536) | — |
| FASTJSON2_BYTES | 24.15 µs (162) | 329.02 µs (2,367) | 5.66 ms (37,022) | — |
| **YALIN_STRING** | 29.22 µs (78) | 322.03 µs (1,046) | 4.74 ms (17,620) | — |
| KOTLINX_STRING | 51.21 µs (339) | 750.00 µs (4,833) | 11.49 ms (75,577) | — |
| FASTJSON2_STRING | 24.57 µs (168) | 357.62 µs (2,461) | 5.66 ms (38,503) | — |

### Infinix X6833B, Android 13, 2026-11-20

All 90 benchmark and equivalence tests passed in a single release run. Clocks were not
locked. Median time (allocations per op):

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---|---|---|---|
| **YALIN_BYTES** | 26.8 µs (127) | 276 µs (1,474) | 3.99 ms (20,019) | 16.81 ms (79,301) |
| **YALIN_STRING** | 30.3 µs (128) | 344 µs (1,475) | 4.81 ms (20,020) | 20.32 ms (79,302) |
| FASTJSON2_BYTES | 25.2 µs (162) | 368 µs (2,392) | 5.51 ms (37,422) | 21.94 ms (149,110) |
| FASTJSON2_STRING | 24.9 µs (168) | 365 µs (2,487) | 5.67 ms (38,919) | 23.49 ms (155,080) |
| JACKSON_BYTES | 45.9 µs (267) | 640 µs (3,916) | 9.86 ms (61,192) | 38.62 ms (243,871) |
| GSON_BYTES | 71.3 µs (401) | 749 µs (5,610) | 11.13 ms (87,495) | 44.91 ms (348,517) |
| KOTLINX_STRING | 52.5 µs (339) | 796 µs (4,884) | 12.15 ms (76,393) | 48.44 ms (304,418) |
| KOTLINX_BYTES | 61.1 µs (340) | 876 µs (4,885) | 13.09 ms (76,394) | 53.76 ms (304,420) |
| MOSHI_BYTES | 68.8 µs (329) | 1.01 ms (4,874) | 16.09 ms (76,397) | 63.78 ms (304,546) |

From 16 KB up, Yalın is faster than every library here: 1.3–1.4× faster than fastjson2, and
2.7× faster than Gson at 1 MB. It also allocates about half as many objects as fastjson2.
At 1 KB it is 6% slower than fastjson2. Starting from a `String` costs another 13–21%
because of the UTF-8 encoding, and that is still faster than fastjson2's `String` path from
16 KB up. A separate UTF-16 parser is not justified yet.

## Read four fields on the Infinix X6833B, 2026-11-23

All 13 scenarios completed in the release run above. Median time
(ART allocations per operation):

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---|---|---|---|
| **YALIN_DOCUMENT** | 10.90 µs (18) | 80.44 µs (18) | 1.25 ms (19) | 5.21 ms (19) |
| **YALIN_PROJECT** | 10.17 µs (16) | 115.39 µs (16) | 1.81 ms (17) | 7.29 ms (17) |
| FASTJSON2_TREE | 25.06 µs (164) | 334.58 µs (2,369) | 5.11 ms (37,024) | 19.98 ms (147,438) |
| JACKSON_TREE | 46.96 µs (268) | 618.65 µs (3,876) | 9.94 ms (60,537) | 38.56 ms (241,138) |
| ORGJSON_TREE | 49.77 µs (323) | 665.47 µs (4,727) | 10.27 ms (74,038) | 41.78 ms (294,914) |
| GSON_TREE | 75.17 µs (404) | 739.05 µs (5,552) | 11.26 ms (86,536) | 44.90 ms (344,828) |
| GSON_MODEL | 85.96 µs (233) | 796.80 µs (3,179) | 12.01 ms (49,441) | 47.45 ms (197,105) |
| MOSHI_MODEL | 50.20 µs (164) | 734.55 µs (2,457) | 11.79 ms (38,531) | 47.56 ms (153,566) |
| KOTLINX_TREE | 61.27 µs (349) | 828.11 µs (4,843) | 12.21 ms (75,587) | 50.20 ms (301,012) |
| KOTLINX_MODEL | 69.53 µs (186) | 923.29 µs (2,654) | 14.30 ms (41,449) | 58.32 ms (165,086) |
| MOSHI_TREE | 69.43 µs (330) | 1.02 ms (4,824) | 16.51 ms (75,580) | 64.52 ms (301,127) |
| JACKSON_MODEL | 93.76 µs (415) | 1.22 ms (5,787) | 19.15 ms (90,354) | 75.96 ms (359,851) |
| FASTJSON2_MODEL | 201.44 µs (1,145) | 2.88 ms (16,199) | 43.87 ms (254,738) | 178.61 ms (1,015,018) |

## Earlier Infinix X6833B projection run

In a single release run on Android 13, all 52 measurements and the equivalence
test passed. These are median times for the two Yalın paths and the fastest
comparison path in this run. See the workload above for what each path does.

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---:|---:|---:|---:|
| `YALIN_PROJECT` | 10.25 µs | 79.69 µs | 1.237 ms | 6.468 ms |
| `YALIN_DOCUMENT` | 10.28 µs | 77.33 µs | 1.201 ms | 6.386 ms |
| `FASTJSON2_TREE` | 26.74 µs | 365.67 µs | 5.692 ms | 22.065 ms |

## Baseline — Infinix X6833B (Helio G99 / MT6789), Android 13, 2026-11-19

All 13 scenarios. Clocks not locked, USB power, single run. Median time
(allocations per op), sorted by 1 MB time.

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---|---|---|---|
| **YALIN_DOCUMENT** | 13.3 µs (18) | 126.5 µs (18) | 2.18 ms (19) | 10.90 ms (19) |
| **YALIN_PROJECT** | 12.8 µs (16) | 133.4 µs (16) | 2.29 ms (17) | 11.63 ms (17) |
| FASTJSON2_TREE | 27.2 µs (164) | 374.5 µs (2,394) | 5.72 ms (37,424) | 22.27 ms (149,112) |
| JACKSON_TREE | 48.6 µs (268) | 651.3 µs (3,917) | 10.32 ms (61,193) | 39.02 ms (243,872) |
| ORGJSON_TREE | 52.5 µs (323) | 689.9 µs (4,777) | 10.62 ms (74,838) | 43.42 ms (298,262) |
| GSON_TREE | 76.6 µs (404) | 817.0 µs (5,613) | 11.99 ms (87,498) | 44.74 ms (348,520) |
| GSON_MODEL | 87.9 µs (233) | 819.2 µs (3,215) | 12.31 ms (50,003) | 49.00 ms (199,120) |
| MOSHI_MODEL | 51.3 µs (164) | 751.3 µs (2,483) | 12.45 ms (38,949) | 49.90 ms (155,310) |
| KOTLINX_TREE | 65.9 µs (349) | 877.7 µs (4,894) | 13.52 ms (76,403) | 53.77 ms (304,429) |
| KOTLINX_MODEL | 71.7 µs (186) | 1.01 ms (2,682) | 15.44 ms (41,965) | 63.75 ms (166,960) |
| MOSHI_TREE | 72.5 µs (330) | 1.08 ms (4,875) | 17.16 ms (76,398) | 66.50 ms (304,547) |
| JACKSON_MODEL | 98.8 µs (415) | 1.29 ms (5,848) | 20.27 ms (91,331) | 79.86 ms (363,936) |
| FASTJSON2_MODEL | 206.9 µs (1,145) | 3.03 ms (16,370) | 45.67 ms (257,491) | 185.51 ms (1,026,541) |

Fastest competitor everywhere is fastjson2 tree: yalin is 2.1× (1 KB), 3.0× (16 KB),
2.6× (256 KB), 2.0× (1 MB) faster; vs Gson tree 4.1× at 1 MB. yalin ≈ 96 MB/s at 1 MB.

## Results — SM-A307FN (Exynos 7904), Android 11, 2026-11-19

All 13 scenarios. Rooted (Magisk), clocks not locked, USB power, single run.
Median time (allocations per op), sorted by 1 MB time.

| Scenario | 1 KB | 16 KB | 256 KB | 1 MB |
|---|---|---|---|---|
| **YALIN_PROJECT** | 35.4 µs (20) | 319.3 µs (20) | 4.73 ms (21) | 18.56 ms (21) |
| **YALIN_DOCUMENT** | 37.7 µs (22) | 322.5 µs (22) | 4.76 ms (23) | 18.56 ms (23) |
| FASTJSON2_TREE | 88.8 µs (220) | 797.8 µs (2,628) | 11.22 ms (40,458) | 46.06 ms (161,094) |
| JACKSON_TREE | 137.7 µs (269) | 1.68 ms (3,919) | 24.97 ms (61,224) | 112.35 ms (243,993) |
| GSON_TREE | 174.7 µs (404) | 1.86 ms (5,612) | 25.69 ms (87,483) | 122.50 ms (348,478) |
| MOSHI_MODEL | 127.0 µs (243) | 1.88 ms (3,720) | 31.51 ms (58,441) | 128.26 ms (233,017) |
| ORGJSON_TREE | 137.6 µs (422) | 2.05 ms (6,212) | 31.95 ms (97,333) | 130.23 ms (387,913) |
| GSON_MODEL | 216.9 µs (234) | 2.25 ms (3,216) | 34.26 ms (50,018) | 140.41 ms (199,200) |
| KOTLINX_TREE | 163.3 µs (351) | 2.26 ms (4,896) | 33.84 ms (76,406) | 145.21 ms (304,433) |
| MOSHI_TREE | 177.3 µs (474) | 2.66 ms (7,067) | 41.25 ms (110,866) | 174.50 ms (441,938) |
| KOTLINX_MODEL | 205.0 µs (189) | 2.81 ms (2,686) | 45.07 ms (41,930) | 195.03 ms (167,085) |
| JACKSON_MODEL | 303.5 µs (416) | 3.94 ms (5,850) | 59.62 ms (91,362) | 261.21 ms (364,058) |
| FASTJSON2_MODEL | 455.1 µs (1,157) | 6.54 ms (16,560) | 99.52 ms (260,485) | 405.79 ms (1,038,488) |

Fastest competitor again fastjson2 tree: yalin 2.4–2.5× faster at every size;
vs Gson tree 6.6× at 1 MB. yalin ≈ 56 MB/s at 1 MB. yalin numbers match the
earlier Gson/Moshi-only run within 1%.

## Cross-device baseline, 2026-11-19 (1 MB, Yalın project)

| Device | SoC | yalin | fastest competitor | speedup |
|---|---|---|---|---|
| Infinix X6833B | Helio G99 (2× A76) | 11.6 ms | 22.3 ms (fastjson2 tree) | 1.9× |
| Samsung SM-A307FN | Exynos 7904 (2× A73) | 18.6 ms | 46.1 ms (fastjson2 tree) | 2.5× |

## Device notes

- Rooted (Magisk) devices: the benchmark library runs `su root id`; Magisk leaves a
  root `sh` holding the output pipe and the runner hangs in `ShellImpl.<clinit>`.
  Kill that `sh` (child of `magiskd`) to continue.
- Gradle needs a JDK 17–21 (`JAVA_HOME`); the system JDK 27 fails with
  "No Java compiler found".
