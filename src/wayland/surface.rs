//! A surface implementation using Wayland surfaces backed by TextureImage.

use crate::base::egl::surface::{EGLBackedSurface, EGLSurfaceTexture};
use crate::Error;

use euclid::default::Size2D;
use std::marker::PhantomData;
use std::os::raw::c_void;
use wayland_sys::client::wl_proxy;
use wayland_sys::egl::{wayland_egl_handle, wl_egl_window};

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
///
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
///
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
#[derive(Debug)]
pub struct Surface(pub(crate) EGLBackedSurface);

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
#[derive(Debug)]
pub struct SurfaceTexture(pub(crate) EGLSurfaceTexture);

/// A wrapper for a Wayland surface, with associated size.
#[derive(Clone)]
pub struct NativeWidget {
    pub(crate) wayland_surface: *mut wl_proxy,
    pub(crate) size: Size2D<i32>,
}

unsafe impl Send for Surface {}

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
