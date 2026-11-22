import Foundation
import IkigaJSON
import Yalin
import ZippyJSON

/// Same datasets as the Android sample: generated product catalogs plus the
/// Vega `cars.json` file, downloaded on each run.
enum Dataset: String, CaseIterable, Identifiable {
  case kb1 = "1 KB", kb16 = "16 KB", kb256 = "256 KB", mb1 = "1 MB", cars = "Cars (net)"
  var id: String { rawValue }
  var targetBytes: Int? {
    switch self {
    case .kb1: 1_024
    case .kb16: 16_384
    case .kb256: 262_144
    case .mb1: 1_048_576
    case .cars: nil
    }
  }
}

struct Timing: Identifiable {
  let name: String
  let milliseconds: Double
  var id: String { name }
}

struct BenchmarkResult {
  let byteCount: Int
  let recordCount: Int
  let downloadMilliseconds: Double?
  let timings: [Timing]  // fastest first
  let summary: [String]
}

private struct Item: Decodable {
  struct Seller: Decodable { let id: Int; let name: String; let rating: Double }
  let id: Int64, sku: String, name: String, price: Double, inStock: Bool, tags: [String], seller: Seller
}

private struct Catalog: Decodable {
  struct Meta: Decodable { let page: Int; let total: Int }
  let meta: Meta
  let items: [Item]
}

private struct Car: Decodable {
  let Name: String
  let Origin: String
}

enum Benchmark {
  static let carsURL = URL(string:
    "https://raw.githubusercontent.com/vega/vega-datasets/192178cf90a83fedccbab80c1f067c29cac6a697/data/cars.json")!
  static let runs = 9

  /// Network call: fetch the Vega cars file.
  static func downloadCars() async throws -> (data: Data, milliseconds: Double) {
    var request = URLRequest(url: carsURL)
    request.timeoutInterval = 10
    request.cachePolicy = .reloadIgnoringLocalCacheData
    let start = DispatchTime.now().uptimeNanoseconds
    let (data, response) = try await URLSession.shared.data(for: request)
    if let http = response as? HTTPURLResponse, http.statusCode != 200 {
      throw URLError(.badServerResponse, userInfo: [NSLocalizedDescriptionKey: "HTTP \(http.statusCode)"])
    }
    return (data, Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
  }

  static func payload(targetBytes: Int) -> (data: Data, count: Int) {
    var items = ""
    var count = 0
    while items.utf8.count + 60 < targetBytes || count == 0 {
      if count > 0 { items += "," }
      let i = count
      let price = Double(i * 37 % 100_000) / 100.0 + 0.5
      let rating = Double(i * 7 % 50) / 10.0
      let sku = String(repeating: "0", count: max(0, 6 - String(i).count)) + String(i)
      items += #"{"id":\#(1_000_000 + i),"sku":"SKU-\#(sku)","#
      items += #""name":"Café \#(i) \"élég\" thé","price":\#(price),"inStock":\#(i % 3 != 0),"#
      items += #""tags":["t\#(i % 5)","t\#(i % 7)","discount"],"#
      items += #""seller":{"id":\#(i % 97),"name":"Vendeur \#(i % 97)","rating":\#(rating)}}"#
      count += 1
    }
    let json = #"{"meta":{"page":1,"total":\#(count),"next":null},"items":[\#(items)]}"#
    return (Data(json.utf8), count)
  }

  /// Runs every parser `runs` times, shifting which one goes first on each
  /// pass (like the Android sample), and returns each one's median in ms.
  private static func measure(_ rows: [(String, () throws -> Void)]) throws -> [Timing] {
    for (_, body) in rows { try body() }  // warm-up
    var samples = Array(repeating: [Double](), count: rows.count)
    for round in 0..<runs {
      for offset in rows.indices {
        let index = (offset + round) % rows.count
        let start = DispatchTime.now().uptimeNanoseconds
        try rows[index].1()
        samples[index].append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
      }
    }
    return rows.indices
      .map { Timing(name: rows[$0].0, milliseconds: samples[$0].sorted()[runs / 2]) }
      .sorted { $0.milliseconds < $1.milliseconds }
  }

  static func run(_ dataset: Dataset, downloaded: (data: Data, milliseconds: Double)?) throws -> BenchmarkResult {
    var sink = 0
    defer { if sink == .min { print(sink) } }  // keep the work observable

    if dataset == .cars, let downloaded {
      let data = downloaded.data
      let tree = try Yalin.parseTree(data)
      let cars = (tree.arrayValue ?? []).prefix(8).map {
        "\($0["Name"]?.stringValue ?? "?") · \($0["Origin"]?.stringValue ?? "?")"
      }
      let timings = try measure([
        ("Yalın parseTree", { sink &+= (try Yalin.parseTree(data)).arrayValue?.count ?? 0 }),
        ("JSONSerialization", { sink &+= (try JSONSerialization.jsonObject(with: data) as? [Any])?.count ?? 0 }),
        ("JSONDecoder (Codable)", { sink &+= try JSONDecoder().decode([Car].self, from: data).count }),
        ("IkigaJSON JSONArray", { sink &+= try JSONArray(data: data).count }),
        ("ZippyJSON (Codable)", { sink &+= try ZippyJSONDecoder().decode([Car].self, from: data).count }),
        ("IkigaJSONDecoder (Codable)", { sink &+= try IkigaJSONDecoder().decode([Car].self, from: data).count }),
      ])
      return BenchmarkResult(byteCount: data.count, recordCount: tree.arrayValue?.count ?? 0,
                             downloadMilliseconds: downloaded.milliseconds, timings: timings, summary: Array(cars))
    }

    let (data, count) = payload(targetBytes: dataset.targetBytes ?? 1_024)
    let paths = ["meta.total", "items[0].name", "items[\(count / 2)].price", "items[\(count - 1)].seller.name"]
    let projected = try Yalin.project(data, paths: paths)
    let summary = [
      "items[0].name: \(try projected.string(at: 1) ?? "")",
      "items[mid].price: \(String(format: "%.2f", try projected.double(at: 2) ?? 0))",
    ]
    let timings = try measure([
      ("Yalın project (4 fields)", {
        let r = try Yalin.project(data, paths: paths)
        sink &+= Int(try r.int64(at: 0) ?? 0) &+ (try r.string(at: 3)?.utf8.count ?? 0)
      }),
      ("Yalın parseTree", {
        if case let .object(o) = try Yalin.parseTree(data) { sink &+= o.count }
      }),
      ("JSONSerialization", { sink &+= (try JSONSerialization.jsonObject(with: data) as? [String: Any])?.count ?? 0 }),
      ("JSONDecoder (Codable)", { sink &+= try JSONDecoder().decode(Catalog.self, from: data).items.count }),
      ("ZippyJSON (Codable)", { sink &+= try ZippyJSONDecoder().decode(Catalog.self, from: data).items.count }),
      ("IkigaJSON JSONObject", { sink &+= try JSONObject(data: data)["meta"]?.object?["total"]?.int ?? 0 }),
      ("IkigaJSONDecoder (Codable)", { sink &+= try IkigaJSONDecoder().decode(Catalog.self, from: data).items.count }),
    ])
    return BenchmarkResult(byteCount: data.count, recordCount: count, downloadMilliseconds: nil,
                           timings: timings, summary: summary)
  }
}
