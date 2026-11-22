// Compares Yalın with Foundation on generated product catalogs.
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

struct CatalogFields: Equatable {
  let total: Int64
  let firstName: String
  let middlePrice: Double
  let lastSeller: String

  var checksum: Int {
    Int(total) &+ firstName.utf8.count &+ Int(middlePrice) &+ lastSeller.utf8.count
  }
}

func yalinFields(_ data: Data, paths: [String]) throws -> CatalogFields {
  let fields = try Yalin.project(data, paths: paths)
  return try CatalogFields(
    total: fields.int64(at: 0)!,
    firstName: fields.string(at: 1)!,
    middlePrice: fields.double(at: 2)!,
    lastSeller: fields.string(at: 3)!
  )
}

func foundationTree(_ data: Data) throws -> [String: Any] {
  try JSONSerialization.jsonObject(with: data) as! [String: Any]
}

func foundationFields(_ data: Data, itemCount: Int) throws -> CatalogFields {
  let tree = try foundationTree(data)
  let meta = tree["meta"] as! [String: Any]
  let items = tree["items"] as! [Any]
  let first = items[0] as! [String: Any]
  let middle = items[itemCount / 2] as! [String: Any]
  let last = items[itemCount - 1] as! [String: Any]
  let seller = last["seller"] as! [String: Any]
  return CatalogFields(
    total: (meta["total"] as! NSNumber).int64Value,
    firstName: first["name"] as! String,
    middlePrice: (middle["price"] as! NSNumber).doubleValue,
    lastSeller: seller["name"] as! String
  )
}

var sink = 0
for (label, size) in [("1 KB", 1_024), ("16 KB", 16_384), ("256 KB", 262_144), ("1 MB", 1_048_576)] {
  let (data, count) = payload(targetBytes: size)
  let paths = ["meta.total", "items[0].name", "items[\(count / 2)].price", "items[\(count - 1)].seller.name"]
  print("\(label) (\(data.count) bytes, \(count) items)")
  let projected = try yalinFields(data, paths: paths)
  let foundation = try foundationFields(data, itemCount: count)
  precondition(projected == foundation, "Projection differs for \(label)")

  let rows: [(String, () throws -> Void)] = [
    ("Yalin.project, 4 fields", { sink &+= try yalinFields(data, paths: paths).checksum }),
    ("JSONSerialization, 4 fields", { sink &+= try foundationFields(data, itemCount: count).checksum }),
    ("Yalin.parseTree", { if case let .object(o) = try Yalin.parseTree(data) { sink &+= o.count } }),
    ("JSONSerialization", { sink &+= try foundationTree(data).count }),
    ("JSONDecoder (Codable)", { sink &+= try JSONDecoder().decode(Catalog.self, from: data).items.count }),
  ]
  for (_, body) in rows { try body() }
  for (name, body) in rows {
    print("  \(name.padding(toLength: 30, withPad: " ", startingAt: 0)) \(String(format: "%10.4f", try measure(body))) ms")
  }
}
print("(checksum \(sink))")
