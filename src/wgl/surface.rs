// surfman/src/platform/windows/wgl/surface.rs
//
//! An implementation of the GPU device for Windows using WGL/Direct3D interoperability.

use crate::renderbuffers::Renderbuffers;
use crate::windows::surface::{Direct3dBinding, Direct3dSurface, Win32Binding, Win32Surface};
use crate::{ContextID, Error, SurfaceInfo};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;
use winapi::shared::minwindef::FALSE;
use winapi::shared::ntdef::HANDLE;
use winapi::um::d3d11::ID3D11Texture2D;
use winapi::um::wingdi;
use winapi::um::winuser;
use wio::com::ComPtr;

/// An implementation of [`crate::Surface`] for WGL platforms.
#[derive(Debug)]
pub enum WglSurface {
    /// A surface displayed in a Win32 window.
    Window(Win32Surface),
    /// A Direct3D 11 texture shared with OpenGL through `WGL_NV_DX_interop`.
    Buffer(Direct3dSurface),
}

/// The WGL objects of a [`Win32Surface`].
pub(crate) struct WglWindowBinding {
    pub(crate) destroyed: bool,
}

/// The WGL objects of a [`Direct3dSurface`].
pub(crate) struct WglBufferBinding {
    pub(crate) d3d11_texture: ComPtr<ID3D11Texture2D>,
    pub(crate) gl_dx_interop_object: HANDLE,
    pub(crate) gl_texture: Option<glow::Texture>,
    pub(crate) gl_framebuffer: Option<glow::Framebuffer>,
    pub(crate) renderbuffers: Renderbuffers,
    pub(crate) destroyed: bool,
}

/// An implementation of [`crate::SurfaceTexture`] for WGL platforms.
pub struct WglSurfaceTexture {
    pub(crate) surface: Direct3dSurface,
    #[allow(dead_code)]
    pub(crate) local_d3d11_texture: ComPtr<ID3D11Texture2D>,
    pub(crate) local_gl_dx_interop_object: HANDLE,
    pub(crate) gl_texture: Option<glow::Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

impl Drop for WglWindowBinding {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Drop for WglBufferBinding {
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

impl Win32Surface {
    pub(crate) fn wgl(&self) -> Result<&WglWindowBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Win32Binding::Wgl(ref binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn wgl_mut(&mut self) -> Result<&mut WglWindowBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Win32Binding::Wgl(ref mut binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn present_wgl(&self) -> Result<(), Error> {
        self.wgl()?;
        let dc = unsafe { winuser::GetDC(self.window) };
        let ok = unsafe { wingdi::SwapBuffers(dc) };
        assert_ne!(ok, FALSE);
        unsafe { winuser::ReleaseDC(self.window, dc) };
        Ok(())
    }
}

impl Direct3dSurface {
    pub(crate) fn wgl(&self) -> Result<&WglBufferBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Direct3dBinding::Wgl(ref binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn wgl_mut(&mut self) -> Result<&mut WglBufferBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Direct3dBinding::Wgl(ref mut binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }
}

impl WglSurface {
    /// Returns the DXGI share handle if it has one.
    #[inline]
    pub fn share_handle(&self) -> Option<HANDLE> {
        match self {
            WglSurface::Window(_) => None,
            WglSurface::Buffer(surface) => Some(surface.share_handle),
        }
    }

    pub(crate) fn context_id(&self) -> ContextID {
        match self {
            WglSurface::Window(surface) => surface.context_id,
            WglSurface::Buffer(surface) => surface.context_id,
        }
    }

    pub(crate) fn is_wgl(&self) -> bool {
        match self {
            WglSurface::Window(surface) => surface.wgl().is_ok(),
            WglSurface::Buffer(surface) => surface.wgl().is_ok(),
        }
    }

    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        match self {
            WglSurface::Window(surface) => surface.size = size,
            WglSurface::Buffer(surface) => surface.size = size,
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        match self {
            WglSurface::Window(surface) => surface.info(),
            WglSurface::Buffer(surface) => surface.info(),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
