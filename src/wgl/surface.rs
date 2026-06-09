// surfman/src/platform/windows/wgl/surface.rs
//
//! An implementation of the GPU device for Windows using WGL/Direct3D interoperability.

use crate::renderbuffers::Renderbuffers;
use crate::{ContextID, Error, SurfaceID};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;
use winapi::shared::minwindef::FALSE;
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::HWND;
use winapi::um::d3d11::ID3D11Texture2D;
use winapi::um::wingdi;
use winapi::um::winuser;
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
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) win32_objects: Win32Objects,
    pub(crate) destroyed: bool,
}

pub(crate) enum Win32Objects {
    Texture {
        d3d11_texture: ComPtr<ID3D11Texture2D>,
        dxgi_share_handle: HANDLE,
        gl_dx_interop_object: HANDLE,
        gl_texture: Option<glow::Texture>,
        gl_framebuffer: Option<glow::Framebuffer>,
        renderbuffers: Renderbuffers,
    },
    Widget {
        window_handle: HWND,
    },
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
    #[allow(dead_code)]
    pub(crate) local_d3d11_texture: ComPtr<ID3D11Texture2D>,
    pub(crate) local_gl_dx_interop_object: HANDLE,
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
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

/// Wraps a Windows `HWND` window handle.
pub struct NativeWidget {
    /// A window handle.
    ///
    /// This can be a top-level window or a control.
    pub window_handle: HWND,
}

impl Surface {
    pub(crate) fn id(&self) -> SurfaceID {
        match self.win32_objects {
            Win32Objects::Texture {
                ref d3d11_texture, ..
            } => SurfaceID((*d3d11_texture).as_raw() as usize),
            Win32Objects::Widget { window_handle } => SurfaceID(window_handle as usize),
        }
    }

    /// Returns the DXGI share handle if it has one.
    #[inline]
    pub fn share_handle(&self) -> Option<HANDLE> {
        match self.win32_objects {
            Win32Objects::Texture {
                dxgi_share_handle, ..
            } => Some(dxgi_share_handle),
            _ => None,
        }
    }

    pub(crate) fn present(&self) -> Result<(), Error> {
        let window_handle = match self.win32_objects {
            Win32Objects::Widget { window_handle } => window_handle,
            _ => return Err(Error::NoWidgetAttached),
        };

        unsafe {
            let dc = winuser::GetDC(window_handle);
            let ok = wingdi::SwapBuffers(dc);
            assert_ne!(ok, FALSE);
            winuser::ReleaseDC(window_handle, dc);
            Ok(())
        }
    }

    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        self.size = size;
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
