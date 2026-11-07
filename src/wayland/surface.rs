//! A surface implementation for Wayland, using EGL window and buffers surfaces.

use crate::base::egl::surface::{EGLBackedSurface, EGLSurfaceTexture, EGLWindowSurface};

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

impl EGLWindowSurface {
    pub(crate) fn resize_for_wayland(&mut self, size: Size2D<i32>) {
        let wayland_egl_window = self.native_window as *mut c_void as *mut wl_egl_window;
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
    }
}
