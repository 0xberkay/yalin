import Foundation
import YalinFFI

/// Entry points. Every input is fully validated before any value is returned.
///
/// Inputs are UTF-8 JSON as any `ContiguousBytes` (`Data`, `[UInt8]`,
/// `UnsafeRawBufferPointer`, …) or a `String`. One-shot calls read the bytes
/// in place, without copying them.
public enum Yalin {
  /// Parses `input` (copied into native memory) into a document for repeated queries.
  public static func parse(_ input: some ContiguousBytes) throws -> YalinDocument {
    try withBytes(input, YalinDocument.open)
  }

  public static func parse(_ input: String) throws -> YalinDocument {
    try withBytes(input, YalinDocument.open)
  }

  /// Parses and looks up `paths` in a single native call, without keeping a document.
  ///
  /// Paths use dotted keys and array indices: `user.name`, `items[3].price`, `[0]`.
  public static func project(_ input: some ContiguousBytes, _ paths: String...) throws -> YalinProjection {
    try project(input, paths: paths)
  }

  public static func project(_ input: some ContiguousBytes, paths: [String]) throws -> YalinProjection {
    try withBytes(input) { try project($0, $1, paths) }
  }

  public static func project(_ input: String, _ paths: String...) throws -> YalinProjection {
    try project(input, paths: paths)
  }

  public static func project(_ input: String, paths: [String]) throws -> YalinProjection {
    try withBytes(input) { try project($0, $1, paths) }
  }

  /// Parses `input` into an immutable ``YalinValue`` tree in one native call.
  ///
  /// Stricter than `parse(_:)`: a repeated object key throws
  /// ``YalinError/parse(_:offset:)`` with ``YalinParseError/duplicateKey``.
  public static func parseTree(_ input: some ContiguousBytes) throws -> YalinValue {
    try withBytes(input, parseTree)
  }

  /// Like `parseTree(_:)`; error offsets are UTF-8 byte offsets.
  public static func parseTree(_ input: String) throws -> YalinValue {
    try withBytes(input, parseTree)
  }

  /// Checks that `input` is a single valid JSON value.
  public static func isValid(_ input: some ContiguousBytes) -> Bool {
    (try? withBytes(input) { try project($0, $1, []) }) != nil
  }

  private static func project(_ ptr: UnsafePointer<UInt8>?, _ len: Int, _ paths: [String]) throws -> YalinProjection {
    let framed = framePaths(paths)
    let raw = framed.withUnsafeBufferPointer { yalin_project(ptr, len, $0.baseAddress, $0.count) }
    return try YalinProjection(paths: paths, buffer: NativeBuffer(raw))
  }

  private static func parseTree(_ ptr: UnsafePointer<UInt8>?, _ len: Int) throws -> YalinValue {
    // The buffer is freed as soon as the tree is built.
    try YalinTreeDecoder.decode(NativeBuffer(yalin_parse_tree(ptr, len)))
  }
}
