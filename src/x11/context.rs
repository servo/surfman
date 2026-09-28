//! OpenGL rendering contexts on X11 via EGL.

use crate::base::egl::context::EGLBackedContext;
pub use crate::base::egl::context::NativeContext;
use crate::Gl;

/// An implementation of [`crate::Context`] for X11.
pub struct X11Context(pub(crate) EGLBackedContext, pub(crate) Gl);
