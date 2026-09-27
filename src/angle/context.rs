//! Wrapper for EGL contexts managed by ANGLE using Direct3D 11 as a backend on Windows.

use super::surface::Surface;
pub use crate::base::egl::context::NativeContext;
use crate::base::egl::surface::ExternalEGLSurfaces;
use crate::context::ContextID;
use crate::egl;
use crate::egl::types::EGLContext;
use crate::surface::Framebuffer;
use crate::Gl;
use std::thread;

/// An implementation of [`crate::Context`] for ANGLE on Windows.
pub struct AngleContext {
    pub(crate) egl_context: EGLContext,
    pub(crate) id: ContextID,
    pub(crate) framebuffer: Framebuffer<Surface, ExternalEGLSurfaces>,
    pub(crate) context_is_owned: bool,
    pub(crate) gl: Gl,
}

impl Drop for AngleContext {
    #[inline]
    fn drop(&mut self) {
        if self.egl_context != egl::NO_CONTEXT && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}
