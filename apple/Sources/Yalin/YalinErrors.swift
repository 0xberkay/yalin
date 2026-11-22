/// JSON value type. ``missing`` means the path selected nothing.
public enum YalinKind: UInt8, Sendable, CaseIterable {
  // Raw values match `kind_code` in crates/yalin-wire/src/codec.rs.
  case missing = 0
  case null = 1
  case bool = 2
  case number = 3
  case string = 4
  case array = 5
  case object = 6
}

/// Why input was rejected. Raw values match `parse_error_code` in crates/yalin-wire/src/codec.rs.
public enum YalinParseError: Int, Sendable, CaseIterable {
  case inputTooLarge = 0
  case unexpectedEndOfInput = 1
  case unexpectedCharacter = 2
  case unterminatedString = 3
  case controlCharacterInString = 4
  case invalidEscape = 5
  case invalidSurrogate = 6
  case invalidUTF8 = 7
  case invalidLiteral = 8
  case invalidNumber = 9
  case depthLimitExceeded = 10
  case trailingCharacters = 11
  /// An object key repeats. Only `Yalin.parseTree(_:)` reports it; paths take the first occurrence.
  case duplicateKey = 12
  case unknown = 255
}

/// Every error Yalın throws.
public enum YalinError: Error, Sendable, Equatable, CustomStringConvertible {
  /// The input is not valid JSON (or exceeds a resource limit). `offset` is a UTF-8 byte offset.
  case parse(YalinParseError, offset: Int)
  /// The path string is malformed; `position` is a byte offset into its UTF-8 form.
  case invalidPath(path: String, position: Int)
  /// A value (or an intermediate value on the path) has a different type than requested.
  case typeMismatch(path: String, expected: YalinKind, found: YalinKind)
  /// A number that does not fit the requested type: fractional or out of range for
  /// `Int64`, or overflowing `Double`.
  case notRepresentable(path: String)
  /// A bug in Yalın (a native panic or a malformed request), never caused by bad JSON.
  case `internal`

  public var description: String {
    switch self {
    case let .parse(error, offset): "\(error) at byte \(offset)"
    case let .invalidPath(path, position): "invalid path '\(path)' at position \(position)"
    case let .typeMismatch(path, expected, found): "'\(path)': expected \(expected), found \(found)"
    case let .notRepresentable(path): "'\(path)' is not representable in the requested type"
    case .internal: "internal Yalın error"
    }
  }

  /// Status code reserved for internal errors (`YALIN_INTERNAL_ERROR`).
  static let internalStatus = Int64.min

  /// Decodes a non-zero status: `-(((offset << 8) | kind) + 1)`.
  static func fromStatus(_ status: Int64) -> YalinError {
    if status == internalStatus || status >= 0 { return .internal }
    let packed = -(status + 1)
    let kind = YalinParseError(rawValue: Int(packed & 0xFF)) ?? .unknown
    return .parse(kind, offset: Int(packed >> 8))
  }
}
