// surfman/src/platform/unix/x11/surface.rs
//
//! A surface implementation using X11 surfaces backed by TextureImage.

use crate::base::egl::surface::{EGLBackedSurface, EGLSurfaceTexture};
use std::marker::PhantomData;

/// An implementation of [`crate::Surface`] for X11 platforms.
#[derive(Debug)]
pub struct X11Surface(pub(crate) EGLBackedSurface);

/// An implementation of [`crate::SurfaceTexture`] for X11 platforms.
#[derive(Debug)]
pub struct X11SurfaceTexture(pub(crate) EGLSurfaceTexture);

unsafe impl Send for X11Surface {}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
