/// Decodes the tree format of crates/yalin-wire/src/tree.rs. Keep both in sync.
///
/// Reads straight from the native buffer. Repeated keys and short strings
/// arrive as slot references, so each is decoded into a `String` only once per
/// tree and later uses share its storage.
struct YalinTreeDecoder {
  private let base: UnsafeRawPointer
  private var pos: Int
  /// Mirrors of the encoder's slot caches, filled by `*_NEW` entries.
  private let keySlots: UnsafeMutablePointer<String>
  private let stringSlots: UnsafeMutablePointer<String>

  /// - Throws: ``YalinError/parse(_:offset:)`` for a parse or duplicate-key failure.
  static func decode(_ buffer: NativeBuffer) throws -> YalinValue {
    let status = buffer.status
    if status != 0 { throw YalinError.fromStatus(status) }
    guard buffer.count >= 10 else { throw YalinError.internal }
    let slots = 1 << Int(buffer.base.load(fromByteOffset: 8, as: UInt8.self))
    let keys = UnsafeMutablePointer<String>.allocate(capacity: slots)
    keys.initialize(repeating: "", count: slots)
    let strings = UnsafeMutablePointer<String>.allocate(capacity: slots)
    strings.initialize(repeating: "", count: slots)
    defer {
      keys.deinitialize(count: slots)
      keys.deallocate()
      strings.deinitialize(count: slots)
      strings.deallocate()
    }
    var decoder = YalinTreeDecoder(base: buffer.base, pos: 9, keySlots: keys, stringSlots: strings)
    let value = decoder.value()
    // The encoder writes exactly one root value.
    guard decoder.pos == buffer.count else { throw YalinError.internal }
    return value
  }

  private mutating func value() -> YalinValue {
    let tag = byte()
    switch tag {
    case Tag.null: return .null
    case Tag.false: return .bool(false)
    case Tag.true: return .bool(true)
    case Tag.int: return .int(int64())
    case Tag.double: return .double(Double(bitPattern: UInt64(bitPattern: int64())))
    case Tag.bigNumber: return .bigNumber(string(count: uint32()))
    case Tag.string: return .string(string(count: uint32()))
    case Tag.stringNew:
      let slot = uint16()
      let s = string(count: Int(byte()))
      stringSlots[slot] = s
      return .string(s)
    case Tag.stringRef: return .string(stringSlots[uint16()])
    case Tag.array:
      let count = uint32()
      var items = [YalinValue]()
      items.reserveCapacity(count)
      for _ in 0..<count { items.append(value()) }
      return .array(items)
    case Tag.object:
      let count = uint32()
      var members = [YalinObject.Element]()
      members.reserveCapacity(count)
      for _ in 0..<count {
        let k = key()
        members.append((k, value()))
      }
      return .object(YalinObject(members: members))
    default:
      preconditionFailure("corrupt Yalın tree at byte \(pos - 1)")
    }
  }

  private mutating func key() -> String {
    switch byte() {
    case Tag.key: return string(count: uint32())
    case Tag.keyNew:
      let slot = uint16()
      let s = string(count: Int(byte()))
      keySlots[slot] = s
      return s
    case Tag.keyRef: return keySlots[uint16()]
    default: preconditionFailure("corrupt Yalın key at byte \(pos - 1)")
    }
  }

  @inline(__always) private mutating func byte() -> UInt8 {
    defer { pos += 1 }
    return base.load(fromByteOffset: pos, as: UInt8.self)
  }

  @inline(__always) private mutating func uint16() -> Int {
    defer { pos += 2 }
    return base.readUInt16(at: pos)
  }

  @inline(__always) private mutating func uint32() -> Int {
    defer { pos += 4 }
    return base.readUInt32(at: pos)
  }

  @inline(__always) private mutating func int64() -> Int64 {
    defer { pos += 8 }
    return base.readInt64(at: pos)
  }

  @inline(__always) private mutating func string(count: Int) -> String {
    defer { pos += count }
    return base.readString(at: pos, count: count)
  }

  /// Tags from crates/yalin-wire/src/tree.rs.
  private enum Tag {
    static let null: UInt8 = 1
    static let `false`: UInt8 = 2
    static let `true`: UInt8 = 3
    static let int: UInt8 = 4
    static let double: UInt8 = 5
    static let bigNumber: UInt8 = 6
    static let string: UInt8 = 7
    static let array: UInt8 = 8
    static let object: UInt8 = 9
    static let stringNew: UInt8 = 12
    static let stringRef: UInt8 = 13
    static let key: UInt8 = 0
    static let keyNew: UInt8 = 1
    static let keyRef: UInt8 = 2
  }
}
