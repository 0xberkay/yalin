import Foundation
import XCTest

@testable import Yalin

/// Mirrors android/yalin-android/src/sharedTest/kotlin/dev/yalin/YalinJsonTest.kt.
final class YalinTests: XCTestCase {
  private let json = Data(
    #"""
    {"user": {"id": 42, "name": "Ada", "email": null, "active": true, "score": 9.5,
              "nick": "Ada\n", "big": 123456789012345678901, "huge": 1e400},
     "items": [{"price": 1}, {"price": 2}, {"price": 3}, {"price": 4.25}],
     "text": "héllo 中文 😀"}
    """#.utf8)

  private func parseError(_ body: () throws -> Any) -> YalinParseError? {
    do {
      _ = try body()
      return nil
    } catch let YalinError.parse(error, _) {
      return error
    } catch {
      XCTFail("unexpected \(error)")
      return nil
    }
  }

  func testTypedGetters() throws {
    let doc = try Yalin.parse(json)
    XCTAssertEqual(try doc.string("user.name"), "Ada")
    XCTAssertEqual(try doc.string("user.nick"), "Ada\n")
    XCTAssertEqual(try doc.string("text"), "héllo 中文 😀")
    XCTAssertEqual(try doc.int64("user.id"), 42)
    XCTAssertEqual(try doc.int("user.id"), 42)
    XCTAssertEqual(try doc.double("user.score"), 9.5)
    XCTAssertEqual(try doc.double("user.id"), 42)
    XCTAssertEqual(try doc.double("items[3].price"), 4.25)
    XCTAssertEqual(try doc.bool("user.active"), true)
    XCTAssertEqual(try doc.kind("items"), .array)
    XCTAssertEqual(try doc.kind(""), .object)
  }

  func testMissingNullAndTypeAreDistinct() throws {
    let doc = try Yalin.parse(json)
    XCTAssertEqual(try doc.kind("user.email"), .null)
    XCTAssertEqual(try doc.kind("user.phone"), .missing)
    XCTAssertNil(try doc.string("user.email"))
    XCTAssertNil(try doc.string("user.phone"))
    XCTAssertNil(try doc.int64("items[9].price"))
    XCTAssertThrowsError(try doc.string("user.id")) {
      XCTAssertEqual($0 as? YalinError, .typeMismatch(path: "user.id", expected: .string, found: .number))
    }
    XCTAssertThrowsError(try doc.int64("items.price")) {
      XCTAssertEqual($0 as? YalinError, .typeMismatch(path: "items.price", expected: .object, found: .array))
    }
    for (path, read) in [
      ("user.score", { try doc.int64("user.score") as Any }),
      ("user.big", { try doc.int64("user.big") as Any }),
      ("user.huge", { try doc.double("user.huge") as Any }),
    ] {
      XCTAssertThrowsError(try read()) { XCTAssertEqual($0 as? YalinError, .notRepresentable(path: path)) }
    }
    XCTAssertThrowsError(try doc.string("user..name")) {
      XCTAssertEqual($0 as? YalinError, .invalidPath(path: "user..name", position: 5))
    }
    XCTAssertEqual(try doc.project("user.huge").numberText(at: 0), "1e400")
    XCTAssertEqual(try doc.project("user.id").numberText(at: 0), "42")
  }

  func testProjection() throws {
    let doc = try Yalin.parse(json)
    let r = try doc.project("user.id", "user.name", "user.active", "user.phone", "user.email")
    XCTAssertEqual(r.count, 5)
    XCTAssertEqual(try r.int64(at: 0), 42)
    XCTAssertEqual(try r.string(at: 1), "Ada")
    XCTAssertEqual(try r.bool(at: 2), true)
    XCTAssertEqual(try r.kind(at: 3), .missing)
    XCTAssertEqual(try r.kind(at: 4), .null)

    let oneShot = try Yalin.project(json, "items[0].price", "items[2].price", "nope")
    XCTAssertEqual(try oneShot.int64(at: 0), 1)
    XCTAssertEqual(try oneShot.int64(at: 1), 3)
    XCTAssertEqual(try oneShot.kind(at: 2), .missing)
    XCTAssertEqual(try Yalin.project(json).count, 0)
    XCTAssertEqual(try Yalin.project(#"{"a":"b"}"#, "a").string(at: 0), "b")
    XCTAssertEqual(try Yalin.project([UInt8](json), paths: ["user.name"]).string(at: 0), "Ada")
  }

  func testParseErrors() {
    XCTAssertThrowsError(try Yalin.parse("[1,  ]")) {
      XCTAssertEqual($0 as? YalinError, .parse(.unexpectedCharacter, offset: 5))
    }
    XCTAssertEqual(parseError { try Yalin.project(#"["\uD800"]"#, "[0]") }, .invalidSurrogate)
    XCTAssertEqual(parseError { try Yalin.parse(Data()) }, .unexpectedEndOfInput)
    XCTAssertEqual(parseError { try Yalin.parseTree("") }, .unexpectedEndOfInput)
    let deep = String(repeating: "[", count: 5000) + String(repeating: "]", count: 5000)
    XCTAssertEqual(parseError { try Yalin.parse(deep) }, .depthLimitExceeded)
    XCTAssertFalse(Yalin.isValid(Data("[1,]".utf8)))
    XCTAssertTrue(Yalin.isValid(json))
  }

  func testTreeHasEveryValue() throws {
    let tree = try Yalin.parseTree(json)
    let root = try XCTUnwrap(tree.objectValue)
    XCTAssertEqual(root.keys, ["user", "items", "text"])
    let user = try XCTUnwrap(root["user"]?.objectValue)
    XCTAssertEqual(user["id"], .int(42))
    XCTAssertEqual(user["name"], .string("Ada"))
    XCTAssertEqual(user["email"], .null)
    XCTAssertEqual(user["active"], .bool(true))
    XCTAssertEqual(user["score"], .double(9.5))
    XCTAssertEqual(user["nick"], .string("Ada\n"))
    XCTAssertEqual(user["big"], .bigNumber("123456789012345678901"))
    XCTAssertEqual(user["huge"], .bigNumber("1e400"))
    XCTAssertEqual(user["big"]?.decimalValue, Decimal(string: "123456789012345678901"))
    XCTAssertNil(user["phone"])
    let items = try XCTUnwrap(root["items"]?.arrayValue)
    XCTAssertEqual(items.count, 4)
    XCTAssertEqual(tree["items"]?[3]?["price"], .double(4.25))
    XCTAssertNil(tree["items"]?[4])
    XCTAssertEqual(root["text"], .string("héllo 中文 😀"))
    XCTAssertEqual(try Yalin.parseTree(String(decoding: json, as: UTF8.self)), tree)
  }

  func testTreeDecodesEveryUtf8Width() throws {
    let text = "a é ı ş 中文 € 😀 \u{0} \u{7FF} \u{800} \u{FFFF} \u{10FFFF}"
    let input = "[\"" + text.replacingOccurrences(of: "\u{0}", with: "\\u0000") + "\"]"
    XCTAssertEqual(try Yalin.parseTree(input), .array([.string(text)]))
    let long = String(repeating: "çğ", count: 500)
    XCTAssertEqual(try Yalin.parseTree("\"\(long)\""), .string(long))
  }

  func testTreeRoots() throws {
    XCTAssertEqual(try Yalin.parseTree("-7"), .int(-7))
    XCTAssertEqual(try Yalin.parseTree("1e2"), .double(100))
    XCTAssertEqual(try Yalin.parseTree(String(Int64.min)), .int(.min))
    XCTAssertEqual(try Yalin.parseTree("\"\""), .string(""))
    XCTAssertEqual(try Yalin.parseTree(" false "), .bool(false))
    XCTAssertEqual(try Yalin.parseTree("[]"), .array([]))
    XCTAssertEqual(try Yalin.parseTree("{}").objectValue?.count, 0)
  }

  func testBigObjectLookupAndOrder() throws {
    let keys = (0..<100).map { "k\($0)" }
    let input = "{" + keys.enumerated().map { "\"\($1)\":\($0)" }.joined(separator: ",") + "}"
    let tree = try XCTUnwrap(Yalin.parseTree(input).objectValue)
    XCTAssertEqual(tree.keys, keys)
    for (i, k) in keys.enumerated() { XCTAssertEqual(tree[k], .int(Int64(i))) }
    XCTAssertNil(tree["k100"])
    XCTAssertEqual(tree.map(\.key), keys)
  }

  func testObjectEqualityIgnoresOrder() throws {
    XCTAssertEqual(try Yalin.parseTree(#"{"a":1,"b":[2]}"#), try Yalin.parseTree(#"{"b":[2],"a":1}"#))
    XCTAssertNotEqual(try Yalin.parseTree(#"{"a":1}"#), try Yalin.parseTree(#"{"a":2}"#))
  }

  func testRepeatedStringsShareSlots() throws {
    let row = #"{"id":"SKU-1","tag":"x","name":"a very long product name that is never cached"}"#
    let input = "[" + Array(repeating: row, count: 300).joined(separator: ",") + "]"
    let rows = try XCTUnwrap(Yalin.parseTree(input).arrayValue)
    XCTAssertEqual(rows.count, 300)
    for r in rows {
      XCTAssertEqual(r["id"], .string("SKU-1"))
      XCTAssertEqual(r["name"], .string("a very long product name that is never cached"))
    }
  }

  func testDuplicateKeys() throws {
    XCTAssertThrowsError(try Yalin.parseTree(#"{"a":{"b":1,"b":2}}"#)) {
      XCTAssertEqual($0 as? YalinError, .parse(.duplicateKey, offset: 12))
    }
    // Path queries take the first occurrence.
    XCTAssertEqual(try Yalin.project(#"{"a":1,"a":2}"#, "a").int64(at: 0), 1)
  }

  func testConcurrentQueries() throws {
    let doc = try Yalin.parse(json)
    DispatchQueue.concurrentPerform(iterations: 64) { i in
      XCTAssertEqual(try? doc.int64("items[\(i % 3)].price"), Int64(i % 3 + 1))
    }
  }

  func testSharedTestdata() throws {
    let root = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent().deletingLastPathComponent()
      .deletingLastPathComponent().deletingLastPathComponent()
      .appendingPathComponent("testdata")
    let fm = FileManager.default
    let valid = try fm.contentsOfDirectory(atPath: root.appendingPathComponent("valid").path)
    let invalid = try fm.contentsOfDirectory(atPath: root.appendingPathComponent("invalid").path)
    XCTAssertFalse(valid.isEmpty)
    XCTAssertFalse(invalid.isEmpty)
    for name in valid {
      let data = try Data(contentsOf: root.appendingPathComponent("valid/\(name)"))
      XCTAssertTrue(Yalin.isValid(data), name)
      XCTAssertNoThrow(try Yalin.parseTree(data), name)
      // Cross-check against Foundation where it can represent the value.
      if !name.contains("big_integer"), !name.contains("numbers"),
        let expected = try? JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed)
      {
        XCTAssertTrue(matches(try Yalin.parseTree(data), expected), name)
      }
    }
    for name in invalid {
      let data = try Data(contentsOf: root.appendingPathComponent("invalid/\(name)"))
      XCTAssertFalse(Yalin.isValid(data), name)
      XCTAssertNotNil(parseError { try Yalin.parseTree(data) }, name)
    }
  }

  private func matches(_ value: YalinValue, _ foundation: Any) -> Bool {
    switch value {
    case .null: return foundation is NSNull
    case let .bool(b): return (foundation as? Bool) == b
    case let .int(i): return (foundation as? NSNumber)?.int64Value == i
    case let .double(d): return (foundation as? NSNumber)?.doubleValue == d
    case .bigNumber: return true
    case let .string(s): return (foundation as? String) == s
    case let .array(items):
      guard let other = foundation as? [Any], other.count == items.count else { return false }
      return zip(items, other).allSatisfy { matches($0, $1) }
    case let .object(object):
      guard let other = foundation as? [String: Any], other.count == object.count else { return false }
      return object.allSatisfy { key, v in other[key].map { matches(v, $0) } ?? false }
    }
  }
}
