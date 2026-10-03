//! A surface implementation using Wayland surfaces backed by TextureImage.

use crate::base::egl::surface::{EGLBackedSurface, EGLSurfaceTexture};
use crate::Error;

use euclid::default::Size2D;
use std::marker::PhantomData;
use std::os::raw::c_void;
use wayland_sys::egl::{wayland_egl_handle, wl_egl_window};

/// An implementation of [`crate::Surface`] for Wayland platforms.
#[derive(Debug)]
pub struct WaylandSurface(pub(crate) EGLBackedSurface);

/// An implementation of [`crate::SurfaceTexture`] for Wayland platforms.
#[derive(Debug)]
pub struct WaylandSurfaceTexture(pub(crate) EGLSurfaceTexture);

unsafe impl Send for WaylandSurface {}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}

impl EGLBackedSurface {
    pub(crate) fn resize_for_wayland(&mut self, size: Size2D<i32>) -> Result<(), Error> {
        let wayland_egl_window = self.native_window()? as *mut c_void as *mut wl_egl_window;
        unsafe {
            (wayland_egl_handle().wl_egl_window_resize)(
                wayland_egl_window,
                size.width,
                size.height,
                0,
                0,
            )
        };
        self.resize(size);
        Ok(())
    }
}
