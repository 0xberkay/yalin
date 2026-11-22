import Foundation

/// An immutable JSON value from `Yalin.parseTree(_:)`.
///
/// An integer that fits `Int64` is ``int(_:)``, a number with a fraction or
/// exponent is ``double(_:)`` unless it overflows `Double`, and any other
/// number is ``bigNumber(_:)`` with its text exactly as written.
public enum YalinValue: Sendable, Equatable {
  case null
  case bool(Bool)
  case int(Int64)
  case double(Double)
  case bigNumber(String)
  case string(String)
  case array([YalinValue])
  case object(YalinObject)

  public var kind: YalinKind {
    switch self {
    case .null: .null
    case .bool: .bool
    case .int, .double, .bigNumber: .number
    case .string: .string
    case .array: .array
    case .object: .object
    }
  }

  public var isNull: Bool { if case .null = self { true } else { false } }

  public var boolValue: Bool? { if case let .bool(v) = self { v } else { nil } }

  public var stringValue: String? { if case let .string(v) = self { v } else { nil } }

  public var int64Value: Int64? { if case let .int(v) = self { v } else { nil } }

  /// Integers convert to the nearest `Double`.
  public var doubleValue: Double? {
    switch self {
    case let .int(v): Double(v)
    case let .double(v): v
    default: nil
    }
  }

  /// Any number as a `Decimal` (big numbers are parsed from their text).
  public var decimalValue: Decimal? {
    switch self {
    case let .int(v): Decimal(v)
    case let .double(v): Decimal(v)
    case let .bigNumber(text): Decimal(string: text, locale: Locale(identifier: "en_US_POSIX"))
    default: nil
    }
  }

  public var arrayValue: [YalinValue]? { if case let .array(v) = self { v } else { nil } }

  public var objectValue: YalinObject? { if case let .object(v) = self { v } else { nil } }

  /// The member named `key` of an object; `nil` for a missing key or a non-object.
  public subscript(key: String) -> YalinValue? {
    if case let .object(object) = self { object[key] } else { nil }
  }

  /// The element at `index` of an array; `nil` when out of range or a non-array.
  public subscript(index: Int) -> YalinValue? {
    guard case let .array(items) = self, items.indices.contains(index) else { return nil }
    return items[index]
  }
}

/// A JSON object. Keys are unique and iterate in document order.
///
/// Lookups scan small objects and use a hash index, built once on first use,
/// for larger ones. Equality ignores member order, like JSON.
public final class YalinObject: @unchecked Sendable, RandomAccessCollection, Equatable {
  public typealias Element = (key: String, value: YalinValue)

  /// Members in document order; one array per object keeps allocations low.
  public let members: [Element]

  /// Objects with more members than this get a hash index.
  private static let linearLimit = 16
  /// Guards `index`; only allocated for objects above `linearLimit`.
  private let lock: NSLock?
  private var index: [String: Int]?

  init(members: [Element]) {
    self.members = members
    self.lock = members.count > Self.linearLimit ? NSLock() : nil
  }

  public var keys: [String] { members.map(\.key) }
  public var values: [YalinValue] { members.map(\.value) }

  public var startIndex: Int { 0 }
  public var endIndex: Int { members.count }

  public subscript(position: Int) -> Element { members[position] }

  public subscript(key: String) -> YalinValue? {
    guard let i = position(of: key) else { return nil }
    return members[i].value
  }

  /// Position of `key` in document order.
  public func position(of key: String) -> Int? {
    guard let lock else { return members.firstIndex { $0.key == key } }
    lock.lock()
    defer { lock.unlock() }
    if index == nil {
      var built = [String: Int](minimumCapacity: keys.count)
      for (i, m) in members.enumerated() { built[m.key] = i }
      index = built
    }
    return index?[key]
  }

  /// The members as a dictionary (order is lost).
  public var dictionary: [String: YalinValue] {
    Dictionary(uniqueKeysWithValues: members.lazy.map { ($0.key, $0.value) })
  }

  public static func == (lhs: YalinObject, rhs: YalinObject) -> Bool {
    if lhs === rhs { return true }
    guard lhs.count == rhs.count else { return false }
    for (key, value) in lhs.members {
      guard let other = rhs[key], other == value else { return false }
    }
    return true
  }
}
