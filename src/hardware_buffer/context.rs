//! OpenGL rendering contexts.

use super::surface::Surface;
use crate::base::egl::surface::ExternalEGLSurfaces;
use crate::context::ContextID;
use crate::egl;
use crate::egl::types::{EGLContext, EGLSurface};
use crate::surface::Framebuffer;
use crate::Gl;
use std::thread;

pub use crate::base::egl::context::NativeContext;

/// An implementation of [`crate::Context`] for Android and OHOS platforms.
pub struct HardwareBufferContext {
    pub(crate) egl_context: EGLContext,
    pub(crate) id: ContextID,
    pub(crate) pbuffer: EGLSurface,
    pub(crate) gl: Gl,
    pub(crate) framebuffer: Framebuffer<Surface, ExternalEGLSurfaces>,
    pub(crate) context_is_owned: bool,
}

impl Drop for HardwareBufferContext {
    #[inline]
    fn drop(&mut self) {
        if self.egl_context != egl::NO_CONTEXT && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}
