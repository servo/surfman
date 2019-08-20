mod surface;
pub use crate::with_egl::gl_context::{NativeGLContext, NativeGLContextHandle};
pub use self::surface::{NativeSurface, NativeSurfaceTexture};
pub(crate) use self::surface::DISPLAY;

// NB: The last three zeros in egl attributes after the egl::EGL_NONE
// are a workaround for workaround buggy implementations.
// Also, when we compare a createxx call with zero, it's equivalent to
// compare it with egl::EGL_NO_XX
