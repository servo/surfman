// surfman/src/platform/windows/wgl/surface.rs
//
//! An implementation of the GPU device for Windows using WGL/Direct3D interoperability.

use crate::renderbuffers::Renderbuffers;
use crate::{ContextID, Error, SurfaceID, SurfaceInfo};

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

/// An implementation of [`crate::Surface`] for WGL platforms.
pub struct WglSurface {
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

/// An implementation of [`crate::SurfaceTexture`] for WGL platforms.
pub struct WglSurfaceTexture {
    pub(crate) surface: WglSurface,
    #[allow(dead_code)]
    pub(crate) local_d3d11_texture: ComPtr<ID3D11Texture2D>,
    pub(crate) local_gl_dx_interop_object: HANDLE,
    pub(crate) gl_texture: Option<glow::Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for WglSurface {}

impl Debug for WglSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Drop for WglSurface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Debug for WglSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl WglSurface {
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

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: match self.win32_objects {
                Win32Objects::Texture { gl_framebuffer, .. } => gl_framebuffer,
                Win32Objects::Widget { .. } => None,
            },
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
