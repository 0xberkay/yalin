/// Results of looking up several paths in one native call, indexed like the
/// requested paths. Decoding is lazy: a string is only created when it is read,
/// straight from the native result buffer.
///
/// Accessors return `nil` for both a missing path and a JSON `null`; use
/// ``kind(at:)`` to tell them apart. A value of another type throws
/// ``YalinError/typeMismatch(path:expected:found:)``; a malformed path throws
/// ``YalinError/invalidPath(path:position:)``.
public struct YalinProjection: Sendable {
  public let paths: [String]
  private let buffer: NativeBuffer
  /// Byte offset of each entry in `buffer`.
  private let offsets: [Int]

  init(paths: [String], buffer: NativeBuffer) throws {
    let status = buffer.status
    if status != 0 { throw YalinError.fromStatus(status) }
    let base = buffer.base
    var offsets = [Int]()
    offsets.reserveCapacity(paths.count)
    var at = 8
    for _ in paths.indices {
      guard at < buffer.count else { throw YalinError.internal }
      offsets.append(at)
      at += Self.entrySize(base, at)
    }
    self.paths = paths
    self.buffer = buffer
    self.offsets = offsets
  }

  public var count: Int { paths.count }

  public func kind(at index: Int) throws -> YalinKind {
    switch try tag(index) {
    case Tag.missing: .missing
    case Tag.null: .null
    case Tag.false, Tag.true: .bool
    case Tag.int, Tag.double, Tag.bigNumber: .number
    case Tag.string: .string
    case Tag.array: .array
    default: .object
    }
  }

  public func string(at index: Int) throws -> String? {
    switch try tag(index) {
    case Tag.missing, Tag.null: return nil
    case Tag.string:
      let at = offsets[index] + 1
      return buffer.base.readString(at: at + 4, count: buffer.base.readUInt32(at: at))
    default: throw try mismatch(index, .string)
    }
  }

  /// Throws ``YalinError/notRepresentable(path:)`` for a fractional number or one outside `Int64`.
  public func int64(at index: Int) throws -> Int64? {
    switch try tag(index) {
    case Tag.missing, Tag.null: return nil
    case Tag.int: return buffer.base.readInt64(at: offsets[index] + 1)
    case Tag.double, Tag.bigNumber: throw YalinError.notRepresentable(path: paths[index])
    default: throw try mismatch(index, .number)
    }
  }

  /// Like ``int64(at:)``; also throws when the value does not fit `Int`.
  public func int(at index: Int) throws -> Int? {
    guard let value = try int64(at: index) else { return nil }
    guard let int = Int(exactly: value) else { throw YalinError.notRepresentable(path: paths[index]) }
    return int
  }

  /// Integers convert to the nearest `Double`. Throws ``YalinError/notRepresentable(path:)``
  /// for a number that overflows `Double`.
  public func double(at index: Int) throws -> Double? {
    switch try tag(index) {
    case Tag.missing, Tag.null: return nil
    case Tag.int: return Double(buffer.base.readInt64(at: offsets[index] + 1))
    case Tag.double: return buffer.base.readDouble(at: offsets[index] + 1)
    case Tag.bigNumber: throw YalinError.notRepresentable(path: paths[index])
    default: throw try mismatch(index, .number)
    }
  }

  public func bool(at index: Int) throws -> Bool? {
    switch try tag(index) {
    case Tag.missing, Tag.null: return nil
    case Tag.true: return true
    case Tag.false: return false
    default: throw try mismatch(index, .bool)
    }
  }

  /// Any number as text: a number outside `Int64` and `Double` exactly as written,
  /// others formatted from their decoded value (so `1E2` reads as `100.0`).
  public func numberText(at index: Int) throws -> String? {
    let at = offsets[index] + 1
    switch try tag(index) {
    case Tag.missing, Tag.null: return nil
    case Tag.int: return String(buffer.base.readInt64(at: at))
    case Tag.double: return String(buffer.base.readDouble(at: at))
    case Tag.bigNumber: return buffer.base.readString(at: at + 4, count: buffer.base.readUInt32(at: at))
    default: throw try mismatch(index, .number)
    }
  }

  /// Tag of a resolvable entry; path and intermediate-type errors throw here.
  private func tag(_ index: Int) throws -> UInt8 {
    let at = offsets[index]
    let base = buffer.base
    let tag = base.load(fromByteOffset: at, as: UInt8.self)
    switch tag {
    case Tag.invalidPath:
      throw YalinError.invalidPath(path: paths[index], position: base.readUInt32(at: at + 1))
    case Tag.typeMismatch:
      throw YalinError.typeMismatch(
        path: paths[index],
        expected: YalinKind(rawValue: base.load(fromByteOffset: at + 1, as: UInt8.self)) ?? .missing,
        found: YalinKind(rawValue: base.load(fromByteOffset: at + 2, as: UInt8.self)) ?? .missing
      )
    default:
      return tag
    }
  }

  private func mismatch(_ index: Int, _ expected: YalinKind) throws -> YalinError {
    .typeMismatch(path: paths[index], expected: expected, found: try kind(at: index))
  }

  private static func entrySize(_ base: UnsafeRawPointer, _ at: Int) -> Int {
    switch base.load(fromByteOffset: at, as: UInt8.self) {
    case Tag.int, Tag.double: 9
    case Tag.bigNumber, Tag.string: 5 + base.readUInt32(at: at + 1)
    case Tag.invalidPath: 5
    case Tag.typeMismatch: 3
    default: 1
    }
  }

  /// Tags from crates/yalin-wire/src/codec.rs.
  private enum Tag {
    static let missing: UInt8 = 0
    static let null: UInt8 = 1
    static let `false`: UInt8 = 2
    static let `true`: UInt8 = 3
    static let int: UInt8 = 4
    static let double: UInt8 = 5
    static let bigNumber: UInt8 = 6
    static let string: UInt8 = 7
    static let array: UInt8 = 8
    static let invalidPath: UInt8 = 10
    static let typeMismatch: UInt8 = 11
  }
}
