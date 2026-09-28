//! OpenGL rendering contexts on Wayland.

use crate::base::egl::context::EGLBackedContext;
pub use crate::base::egl::context::NativeContext;
use crate::Gl;

/// An implementation of [`crate::Context`] for Wayland.
pub struct WaylandContext(pub(crate) EGLBackedContext, pub(crate) Gl);
