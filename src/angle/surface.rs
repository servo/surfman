//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use super::context::ContextDescriptor;
use super::device::Device;
use crate::base::egl::device::EGL_FUNCTIONS;
use crate::context::ContextID;
use crate::egl::types::EGLNativeWindowType;
use crate::egl::types::EGLSurface;
use crate::egl::{self};
use crate::{Error, SurfaceID};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;
use winapi::shared::dxgi::IDXGIKeyedMutex;
use winapi::um::d3d11;
use winapi::um::winnt::HANDLE;
use wio::com::ComPtr;

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
pub struct Surface {
    pub(crate) egl_surface: EGLSurface,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) context_descriptor: ContextDescriptor,
    pub(crate) win32_objects: Win32Objects,
}

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) local_egl_surface: EGLSurface,
    pub(crate) local_keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
    pub(crate) gl_texture: Option<glow::Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.egl_surface != egl::NO_SURFACE && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

pub(crate) enum Win32Objects {
    Window,
    Pbuffer {
        share_handle: HANDLE,
        synchronization: Synchronization,
        // We keep a reference to the ComPtr in order to keep its refcount from becoming zero
        texture: Option<ComPtr<d3d11::ID3D11Texture2D>>,
    },
}

pub(crate) enum Synchronization {
    KeyedMutex(ComPtr<IDXGIKeyedMutex>),
    GLFinish,
    None,
}

/// Wraps an `EGLNativeWindowType`
#[repr(C)]
pub struct NativeWidget {
    /// A native window
    ///
    /// This can be a top-level window or a control.
    pub egl_native_window: EGLNativeWindowType,
}

impl Surface {
    #[inline]
    pub(crate) fn id(&self) -> SurfaceID {
        SurfaceID(self.egl_surface as usize)
    }

    #[inline]
    pub(crate) fn uses_gl_finish(&self) -> bool {
        match self.win32_objects {
            Win32Objects::Pbuffer {
                synchronization: Synchronization::GLFinish,
                ..
            } => true,
            _ => false,
        }
    }

    /// Returns the DXGI share handle if it has one.
    #[inline]
    pub fn share_handle(&self) -> Option<HANDLE> {
        match self.win32_objects {
            Win32Objects::Pbuffer { share_handle, .. } => Some(share_handle),
            _ => None,
        }
    }

    pub(crate) fn present(&self, device: &Device) -> Result<(), Error> {
        match self.win32_objects {
            Win32Objects::Window { .. } => {}
            _ => return Err(Error::NoWidgetAttached),
        }

        EGL_FUNCTIONS.with(|egl| unsafe {
            let ok = egl.SwapBuffers(device.egl_display, self.egl_surface);
            assert_ne!(ok, egl::FALSE);
            Ok(())
        })
    }

    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        self.size = size;
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
