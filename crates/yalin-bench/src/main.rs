//! Core-only benchmark (no JNI, no JVM). Same workload as `android/benchmark`
//! (`Payloads.kt`): parse + fully validate, then extract 4 values.
//!
//! ```text
//! yalin-bench [--runs N] [--ms MS] [--filter SUBSTR]   # table
//! yalin-bench --spin SCENARIO SIZE SECONDS              # hot loop for simpleperf
//! ```

use std::hint::black_box;
use std::time::{Duration, Instant};

use serde::de::IgnoredAny;
use sonic_rs::{JsonContainerTrait, JsonValueTrait};

const SIZES: [(&str, usize); 4] = [("1KB", 1 << 10), ("16KB", 16 << 10), ("256KB", 256 << 10), ("1MB", 1 << 20)];

struct Payload {
    label: &'static str,
    bytes: Vec<u8>,
    paths: [String; 4],
}

/// Byte-for-byte port of `Payloads.generate` (Kotlin `Double.toString` == Rust `{:?}` here).
fn payload(label: &'static str, target: usize) -> Payload {
    let mut items = String::new();
    let mut count = 0usize;
    while items.len() + 60 < target || count == 0 {
        if count > 0 {
            items.push(',');
        }
        let i = count;
        let price = (i * 37 % 100_000) as f64 / 100.0 + 0.5;
        let rating = (i * 7 % 50) as f64 / 10.0;
        items += &format!(
            r#"{{"id":{},"sku":"SKU-{:06}","name":"Café {i} \"élég\" thé","price":{price:?},"inStock":{},"tags":["t{}","t{}","discount"],"seller":{{"id":{},"name":"Vendeur {}","rating":{rating:?}}}}}"#,
            1_000_000 + i,
            i,
            i % 3 != 0,
            i % 5,
            i % 7,
            i % 97,
            i % 97,
        );
        count += 1;
    }
    let json = format!(r#"{{"meta":{{"page":1,"total":{count},"next":null}},"items":[{items}]}}"#);
    let (mid, last) = (count / 2, count - 1);
    Payload {
        label,
        bytes: json.into_bytes(),
        paths: [
            "meta.total".into(),
            "items[0].name".into(),
            format!("items[{mid}].price"),
            format!("items[{last}].seller.name"),
        ],
    }
}

#[derive(Debug, PartialEq)]
struct Extracted {
    total: i64,
    first_name: String,
    mid_price: f64,
    last_seller: String,
}

type Scenario = fn(&Payload) -> Option<Extracted>;

const SCENARIOS: &[(&str, Scenario)] = &[
    ("utf8_only", |p| std::str::from_utf8(&p.bytes).ok().map(|_| dummy())),
    ("yalin_validate", |p| yalin_core::validate(&p.bytes).ok().map(|_| dummy())),
    ("yalin_extract", yalin_extract),
    // What `YalinJson.parseTree` does in native code: parse, walk, encode the tree.
    ("yalin_tree", |p| (yalin::tree::encode(&p.bytes)[..8] == [0; 8]).then(dummy)),
    ("serde_ignored", |p| serde_json::from_slice::<IgnoredAny>(&p.bytes).ok().map(|_| dummy())),
    ("serde_value", serde_value),
    ("sonic_ignored", |p| sonic_rs::from_slice::<IgnoredAny>(&p.bytes).ok().map(|_| dummy())),
    ("sonic_value", sonic_value),
];

fn dummy() -> Extracted {
    Extracted { total: 0, first_name: String::new(), mid_price: 0.0, last_seller: String::new() }
}

fn yalin_extract(p: &Payload) -> Option<Extracted> {
    let doc = yalin_core::Document::parse(p.bytes.as_slice()).ok()?;
    Some(Extracted {
        total: doc.get(&p.paths[0]).ok()?.as_i64().ok()?,
        first_name: doc.get(&p.paths[1]).ok()?.as_str().ok()?.into_owned(),
        mid_price: doc.get(&p.paths[2]).ok()?.as_f64().ok()?,
        last_seller: doc.get(&p.paths[3]).ok()?.as_str().ok()?.into_owned(),
    })
}

fn indices(p: &Payload) -> (usize, usize) {
    let n = serde_json::from_slice::<serde_json::Value>(&p.bytes).unwrap()["items"].as_array().unwrap().len();
    (n / 2, n - 1)
}

fn serde_value(p: &Payload) -> Option<Extracted> {
    let v: serde_json::Value = serde_json::from_slice(&p.bytes).ok()?;
    let items = v["items"].as_array()?;
    let (mid, last) = (items.len() / 2, items.len() - 1);
    Some(Extracted {
        total: v["meta"]["total"].as_i64()?,
        first_name: items[0]["name"].as_str()?.to_owned(),
        mid_price: items[mid]["price"].as_f64()?,
        last_seller: items[last]["seller"]["name"].as_str()?.to_owned(),
    })
}

fn sonic_value(p: &Payload) -> Option<Extracted> {
    let v: sonic_rs::Value = sonic_rs::from_slice(&p.bytes).ok()?;
    let items = v.get("items")?.as_array()?;
    let (mid, last) = (items.len() / 2, items.len() - 1);
    Some(Extracted {
        total: v.get("meta")?.get("total")?.as_i64()?,
        first_name: items.get(0)?.get("name")?.as_str()?.to_owned(),
        mid_price: items.get(mid)?.get("price")?.as_f64()?,
        last_seller: items.get(last)?.get("seller")?.get("name")?.as_str()?.to_owned(),
    })
}

/// Runs `f` repeatedly for about `budget`; returns ns per call.
fn sample(p: &Payload, f: Scenario, budget: Duration) -> f64 {
    let mut iters = 0u64;
    let start = Instant::now();
    loop {
        black_box(f(black_box(p)));
        iters += 1;
        let e = start.elapsed();
        if e >= budget {
            return e.as_nanos() as f64 / iters as f64;
        }
    }
}

fn fmt_ns(ns: f64) -> String {
    if ns < 1e3 {
        format!("{ns:.0} ns")
    } else if ns < 1e6 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{:.3} ms", ns / 1e6)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let payloads: Vec<Payload> = SIZES.iter().map(|&(l, s)| payload(l, s)).collect();

    // Sanity: every real extractor agrees with serde_json.
    for p in &payloads {
        let expected = serde_value(p).expect("serde_json parses payload");
        assert_eq!(indices(p), (p.paths[2][6..].split(']').next().unwrap().parse().unwrap(), expected_last(p)));
        assert_eq!(yalin_extract(p).as_ref(), Some(&expected), "yalin {}", p.label);
        assert_eq!(sonic_value(p).as_ref(), Some(&expected), "sonic {}", p.label);
    }

    if let Some(i) = args.iter().position(|a| a == "--spin") {
        let (name, size, secs) = (&args[i + 1], &args[i + 2], args[i + 3].parse::<u64>().unwrap());
        let f = SCENARIOS.iter().find(|(n, _)| n == name).expect("scenario").1;
        let p = payloads.iter().find(|p| p.label == size).expect("size");
        let ns = sample(p, f, Duration::from_secs(secs));
        println!("{name} {size}: {} per op", fmt_ns(ns));
        return;
    }

    let runs: usize = opt("--runs").map_or(11, |v| v.parse().unwrap());
    let budget = Duration::from_millis(opt("--ms").map_or(200, |v| v.parse().unwrap()));
    let filter = opt("--filter").unwrap_or_default();

    println!("runs={runs} x {budget:?}; median [p10..p90] per op, MB/s at median (MB = 2^20 B)");
    println!("{:<15} {:>6} {:>12} {:>27} {:>9}", "scenario", "size", "median", "p10..p90", "MB/s");
    for p in &payloads {
        for &(name, f) in SCENARIOS.iter().filter(|(n, _)| n.contains(filter.as_str())) {
            sample(p, f, budget / 2); // warm-up (caches, frequency ramp)
            let mut v: Vec<f64> = (0..runs).map(|_| sample(p, f, budget)).collect();
            v.sort_by(f64::total_cmp);
            let q = |x: f64| v[((v.len() - 1) as f64 * x).round() as usize];
            let mbs = p.bytes.len() as f64 / (1 << 20) as f64 / (q(0.5) / 1e9);
            println!(
                "{name:<15} {:>6} {:>12} {:>27} {mbs:>9.1}",
                p.label,
                fmt_ns(q(0.5)),
                format!("[{} .. {}]", fmt_ns(q(0.1)), fmt_ns(q(0.9))),
            );
        }
    }
}

fn expected_last(p: &Payload) -> usize {
    p.paths[3][6..].split(']').next().unwrap().parse().unwrap()
}
