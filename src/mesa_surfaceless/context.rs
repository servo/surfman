//! OpenGL rendering contexts on surfaceless Mesa.

use crate::base::egl::context::EGLBackedContext;
pub use crate::base::egl::context::NativeContext;
use crate::Gl;

/// An implementation of [`crate::Context`] for Surfaceless Mesa.
pub struct SurfacelessMesaContext(pub(crate) EGLBackedContext, pub(crate) Gl);
