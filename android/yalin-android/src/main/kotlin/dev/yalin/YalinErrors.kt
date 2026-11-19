package dev.yalin

/** JSON value type. [MISSING] means the path selected nothing. */
enum class YalinKind {
  MISSING,
  NULL,
  BOOLEAN,
  NUMBER,
  STRING,
  ARRAY,
  OBJECT,
}

/** Why input was rejected. Ordinals match `parse_error_code` in codec.rs. */
enum class YalinParseError {
  INPUT_TOO_LARGE,
  UNEXPECTED_EOF,
  UNEXPECTED_CHARACTER,
  UNTERMINATED_STRING,
  CONTROL_CHARACTER_IN_STRING,
  INVALID_ESCAPE,
  INVALID_SURROGATE,
  INVALID_UTF8,
  INVALID_LITERAL,
  INVALID_NUMBER,
  DEPTH_LIMIT_EXCEEDED,
  TRAILING_CHARACTERS,
  UNKNOWN,
}

/** The input is not valid JSON (or exceeds a resource limit). [offset] is a byte offset. */
class YalinParseException(val error: YalinParseError, val offset: Long) :
  RuntimeException("$error at byte $offset") {
  internal companion object {
    /** Decodes `-(((offset << 8) | kind) + 1)`. */
    fun fromCode(code: Long): YalinParseException {
      val packed = -(code + 1)
      val kind = (packed and 0xFF).toInt()
      return YalinParseException(
        YalinParseError.entries.getOrElse(kind) { YalinParseError.UNKNOWN },
        packed ushr 8,
      )
    }
  }
}

/** The path string is malformed; [position] is a byte offset into its UTF-8 form. */
class YalinPathException(val path: String, val position: Int) :
  IllegalArgumentException("invalid path '$path' at position $position")

/** A value (or an intermediate value on the path) has a different type than requested. */
class YalinTypeException(val path: String, val expected: YalinKind, val found: YalinKind) :
  RuntimeException("'$path': expected $expected, found $found")
