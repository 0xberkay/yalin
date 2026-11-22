// Compares Yalın with Foundation on the product catalog of
// android/shared-benchmark (same payloads and paths as the Android benchmark).
//
//   swift run -c release yalin-benchmark
//
// Numbers from a Mac or a simulator are not device numbers; for those, call
// the same code from an app on an iPhone in a Release build.
import Foundation
import Yalin

func payload(targetBytes: Int) -> (data: Data, count: Int) {
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
  return (Data(#"{"meta":{"page":1,"total":\#(count),"next":null},"items":[\#(items)]}"#.utf8), count)
}

/// Median wall time of `body` in milliseconds.
func measure(_ body: () throws -> Void) rethrows -> Double {
  let deadline = Date().addingTimeInterval(0.5)
  var samples = [Double]()
  repeat {
    let start = DispatchTime.now().uptimeNanoseconds
    try body()
    samples.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
  } while Date() < deadline || samples.count < 5
  samples.sort()
  return samples[samples.count / 2]
}

struct Item: Decodable {
  struct Seller: Decodable { let id: Int; let name: String; let rating: Double }
  let id: Int64, sku: String, name: String, price: Double, inStock: Bool, tags: [String], seller: Seller
}
struct Catalog: Decodable {
  struct Meta: Decodable { let page: Int; let total: Int }
  let meta: Meta
  let items: [Item]
}

var sink = 0
for (label, size) in [("1 KB", 1_024), ("16 KB", 16_384), ("256 KB", 262_144), ("1 MB", 1_048_576)] {
  let (data, count) = payload(targetBytes: size)
  let paths = ["meta.total", "items[0].name", "items[\(count / 2)].price", "items[\(count - 1)].seller.name"]
  print("\(label) (\(data.count) bytes, \(count) items)")
  let rows: [(String, () throws -> Void)] = [
    ("Yalin.project, 4 fields", {
      let r = try Yalin.project(data, paths: paths)
      sink &+= Int(try r.int64(at: 0)!) &+ (try r.string(at: 3)!.utf8.count)
    }),
    ("Yalin.parseTree", { if case let .object(o) = try Yalin.parseTree(data) { sink &+= o.count } }),
    ("JSONSerialization", {
      sink &+= (try JSONSerialization.jsonObject(with: data) as! [String: Any]).count
    }),
    ("JSONDecoder (Codable)", { sink &+= try JSONDecoder().decode(Catalog.self, from: data).items.count }),
  ]
  for (name, body) in rows {
    print("  \(name.padding(toLength: 26, withPad: " ", startingAt: 0)) \(String(format: "%9.3f", try measure(body))) ms")
  }
}
print("(checksum \(sink))")
