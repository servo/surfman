//! surfman/surfman/src/platform/unix/generic/surface.rs
//!
//! Wrapper for EGL surfaces on Mesa.

use crate::base::egl::surface::EGLSurfaceTexture;

use std::marker::PhantomData;

/// An implementation of [`crate::SurfaceTexture`] for Surfaceless Mesa.
#[derive(Debug)]
pub struct SurfacelessMesaSurfaceTexture(pub(crate) EGLSurfaceTexture);

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
