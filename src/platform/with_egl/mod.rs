mod gl_context;
#[cfg(target_os = "windows")]
#[path = "surface_angle.rs"]
mod surface;
#[cfg(not(target_os = "windows"))]
#[path = "surface_native.rs"]
mod surface;
pub use self::gl_context::{NativeGLContext, NativeGLContextHandle};
pub use self::surface::{Display, NativeDisplay, Surface, SurfaceTexture};

// NB: The last three zeros in egl attributes after the egl::EGL_NONE
// are a workaround for workaround buggy implementations.
// Also, when we compare a createxx call with zero, it's equivalent to
// compare it with egl::EGL_NO_XX
