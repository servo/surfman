//! surfman/surfman/src/platform/unix/generic/surface.rs
//!
//! Wrapper for EGL surfaces on Mesa.

use crate::base::egl::surface::{EGLBackedSurface, EGLSurfaceTexture};

use std::marker::PhantomData;

/// An implementation of [`crate::Surface`] for Surfaceless Mesa.
#[derive(Debug)]
pub struct SurfacelessMesaSurface(pub(crate) EGLBackedSurface);

/// An implementation of [`crate::SurfaceTexture`] for Surfaceless Mesa.
#[derive(Debug)]
pub struct SurfacelessMesaSurfaceTexture(pub(crate) EGLSurfaceTexture);

unsafe impl Send for SurfacelessMesaSurface {}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
