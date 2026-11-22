import YalinFFI

/// A validated JSON document held in native memory, freed when the last
/// reference goes away.
///
/// Thread-safe: the document is immutable, so queries may run concurrently
/// from several threads.
///
/// Path syntax: `user.name`, `items[3].price`, `[0]`; the empty path selects
/// the root. Duplicate keys: the first occurrence wins.
public final class YalinDocument: @unchecked Sendable {
  // Immutable and only freed in deinit, so it can never be used after free.
  private let handle: OpaquePointer

  private init(handle: OpaquePointer) {
    self.handle = handle
  }

  deinit { yalin_document_free(handle) }

  static func open(_ ptr: UnsafePointer<UInt8>?, _ len: Int) throws -> YalinDocument {
    var handle: OpaquePointer?
    let status = yalin_document_parse(ptr, len, &handle)
    guard status == 0, let handle else {
      throw status == 0 ? YalinError.internal : YalinError.fromStatus(status)
    }
    return YalinDocument(handle: handle)
  }

  /// Looks up all `paths` in one native call.
  public func project(_ paths: String...) throws -> YalinProjection {
    try project(paths: paths)
  }

  public func project(paths: [String]) throws -> YalinProjection {
    let framed = framePaths(paths)
    let raw = withExtendedLifetime(self) {
      framed.withUnsafeBufferPointer { yalin_document_query(handle, $0.baseAddress, $0.count) }
    }
    return try YalinProjection(paths: paths, buffer: NativeBuffer(raw))
  }

  public func kind(_ path: String) throws -> YalinKind { try project(path).kind(at: 0) }

  public func string(_ path: String) throws -> String? { try project(path).string(at: 0) }

  public func int64(_ path: String) throws -> Int64? { try project(path).int64(at: 0) }

  public func int(_ path: String) throws -> Int? { try project(path).int(at: 0) }

  public func double(_ path: String) throws -> Double? { try project(path).double(at: 0) }

  public func bool(_ path: String) throws -> Bool? { try project(path).bool(at: 0) }
}
