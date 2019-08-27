//! A stub implementation to allow this crate to compile on headless or unsupported platforms.
//!
//! Calling any methods on these objects will fail at runtime.

mod gl_context;
mod surface;

pub use self::gl_context::NativeGLContext;
pub use self::surface::{Surface, SurfaceTexture};
