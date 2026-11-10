// surfman/src/platform/unix/x11/surface.rs
//
//! A surface implementation using X11 surfaces backed by TextureImage.

use crate::base::egl::surface::{EGLSurfaceTexture, EGLWindowSurface};
use std::marker::PhantomData;

/// An implementation of [`crate::WindowSurface`] for X11 platforms.
#[derive(Debug)]
pub struct X11WindowSurface(pub(crate) EGLWindowSurface);

/// An implementation of [`crate::SurfaceTexture`] for X11 platforms.
#[derive(Debug)]
pub struct X11SurfaceTexture(pub(crate) EGLSurfaceTexture);

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
