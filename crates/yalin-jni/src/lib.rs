//! JNI entry points for `dev.yalin.YalinNative` (Kotlin).
//!
//! Ownership rules (see `docs/jni-ownership.md`):
//! - Input bytes are always **copied** into Rust during the call; no pointer
//!   into a Java array or buffer outlives the call.
//! - Documents live in [`registry`]; Java holds only an opaque handle.
//! - Every entry point runs inside `with_env`, which catches panics; Rust
//!   errors and panics surface as `java.lang.RuntimeException`. Expected
//!   outcomes (bad JSON, missing fields, …) are encoded in return values
//!   ([`codec`]), never thrown from native code.

mod codec;
mod registry;
pub mod tree;

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JByteBuffer, JClass};
use jni::sys::{jint, jlong};
use yalin_core::Document;

use codec::ProtocolError;

#[derive(Debug)]
enum BridgeError {
    Jni(jni::errors::Error),
    Protocol(ProtocolError),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jni(e) => write!(f, "{e}"),
            Self::Protocol(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<jni::errors::Error> for BridgeError {
    fn from(e: jni::errors::Error) -> Self {
        Self::Jni(e)
    }
}

impl From<ProtocolError> for BridgeError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}

/// Parses owned bytes and registers the document: a positive handle, or a
/// negative parse error code.
fn open(bytes: Vec<u8>) -> Result<jlong, BridgeError> {
    match Document::parse(bytes) {
        Ok(doc) => Ok(registry::global()
            .insert(doc)
            .ok_or(ProtocolError("too many open documents"))?),
        Err(e) => Ok(codec::parse_error_code(&e)),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yalin_YalinNative_parse<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    input: JByteArray<'l>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, BridgeError> { open(env.convert_byte_array(&input)?) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Copies `length` bytes at `offset` of a direct buffer. The Java side must not
/// mutate that region during the call.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yalin_YalinNative_parseDirect<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    buffer: JByteBuffer<'l>,
    offset: jint,
    length: jint,
) -> jlong {
    env.with_env(|env| -> Result<jlong, BridgeError> {
        let address = env.get_direct_buffer_address(&buffer)?;
        let capacity = env.get_direct_buffer_capacity(&buffer)?;
        let (Ok(offset), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
            return Err(ProtocolError("negative offset or length").into());
        };
        if offset.checked_add(length).is_none_or(|end| end > capacity) {
            return Err(ProtocolError("range outside direct buffer").into());
        }
        // SAFETY: `address` is the non-null base of a live direct buffer of
        // `capacity` bytes (checked by jni-rs), the range is in bounds, and the
        // bytes are copied before this call returns.
        let bytes = unsafe { std::slice::from_raw_parts(address.add(offset), length) }.to_vec();
        open(bytes)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yalin_YalinNative_query<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    handle: jlong,
    paths: JByteArray<'l>,
) -> JByteArray<'l> {
    env.with_env(|env| -> Result<JByteArray<'_>, BridgeError> {
        // Clone the Arc and release the lock before doing any work.
        let Some(doc) = registry::global().get(handle) else {
            return Ok(JByteArray::default()); // null: closed or invalid handle
        };
        let paths = env.convert_byte_array(&paths)?;
        let out = codec::query(&doc, &paths)?;
        Ok(env.byte_array_from_slice(&out)?)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// One-shot parse + projection without registering a document.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yalin_YalinNative_project<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    input: JByteArray<'l>,
    paths: JByteArray<'l>,
) -> JByteArray<'l> {
    env.with_env(|env| -> Result<JByteArray<'_>, BridgeError> {
        let input = env.convert_byte_array(&input)?;
        let paths = env.convert_byte_array(&paths)?;
        let out = match Document::parse(input.as_slice()) {
            Ok(doc) => codec::query(&doc, &paths)?,
            Err(e) => codec::parse_failure(&e),
        };
        Ok(env.byte_array_from_slice(&out)?)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// One-shot parse into an encoded tree ([`tree`]), without registering a document.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yalin_YalinNative_parseTree<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    input: JByteArray<'l>,
) -> JByteArray<'l> {
    env.with_env(|env| -> Result<JByteArray<'_>, BridgeError> {
        let input = env.convert_byte_array(&input)?;
        Ok(env.byte_array_from_slice(&tree::encode(&input))?)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Idempotent: closing an unknown or already-closed handle does nothing.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yalin_YalinNative_close<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    handle: jlong,
) {
    env.with_env(|_env| -> Result<(), BridgeError> {
        // Drop the document outside the lock.
        let removed = registry::global().take(handle);
        drop(removed);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
