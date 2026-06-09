//! OpenGL rendering contexts.

use super::surface::Surface;
use crate::base::egl::surface::ExternalEGLSurfaces;
use crate::context::ContextID;
use crate::egl;
use crate::egl::types::{EGLContext, EGLSurface};
use crate::surface::Framebuffer;
use crate::Gl;
use std::thread;

pub use crate::base::egl::context::{ContextDescriptor, NativeContext};

/// Represents an OpenGL rendering context.
///
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
///
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
///
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
///
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub struct Context {
    pub(crate) egl_context: EGLContext,
    pub(crate) id: ContextID,
    pub(crate) pbuffer: EGLSurface,
    pub(crate) gl: Gl,
    pub(crate) framebuffer: Framebuffer<Surface, ExternalEGLSurfaces>,
    pub(crate) context_is_owned: bool,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if self.egl_context != egl::NO_CONTEXT && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}
