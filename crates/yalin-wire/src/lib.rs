//! Binary result formats shared by the platform bindings.
//!
//! Both `yalin-jni` (decoded by Kotlin) and `yalin-ffi` (decoded by Swift)
//! return these buffers, so one encoder serves every platform:
//!
//! - [`codec`]: path requests, projection results and parse error codes.
//! - [`tree`]: the whole-tree format.

pub mod codec;
pub mod tree;
