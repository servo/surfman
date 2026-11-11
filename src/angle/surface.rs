//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use super::device::AngleDevice;
use crate::base::egl::device::EGL_FUNCTIONS;
use crate::context::ContextID;
use crate::egl::types::EGLSurface;
use crate::egl::{self};
use crate::windows::surface::{Direct3dBinding, Direct3dSurface, Win32Binding, Win32Surface};
use crate::{EglContextDescriptor, Error, SurfaceInfo};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;
use winapi::shared::dxgi::IDXGIKeyedMutex;
use winapi::um::d3d11;
use winapi::um::winnt::HANDLE;
use wio::com::ComPtr;

/// An implementation of [`crate::Surface`] for Angle on Windows.
#[derive(Debug)]
pub enum AngleSurface {
    /// A surface displayed in a Win32 window.
    Window(Win32Surface),
    /// A pbuffer surface backed by a Direct3D 11 texture.
    Buffer(Direct3dSurface),
}

/// The ANGLE objects of a [`Win32Surface`].
pub(crate) struct AngleWindowBinding {
    pub(crate) egl_surface: EGLSurface,
}

/// The ANGLE objects of a [`Direct3dSurface`].
pub(crate) struct AngleBufferBinding {
    pub(crate) egl_surface: EGLSurface,
    pub(crate) context_descriptor: EglContextDescriptor,
    pub(crate) synchronization: Synchronization,
    // We keep a reference to the ComPtr in order to keep its refcount from becoming zero
    pub(crate) texture: Option<ComPtr<d3d11::ID3D11Texture2D>>,
}

/// An implementation of [`crate::SurfaceTexture`] for Angle on Windows.
pub struct AngleSurfaceTexture {
    pub(crate) surface: Direct3dSurface,
    pub(crate) local_egl_surface: EGLSurface,
    pub(crate) local_keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
    pub(crate) gl_texture: Option<glow::Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

impl Drop for AngleWindowBinding {
    fn drop(&mut self) {
        if self.egl_surface != egl::NO_SURFACE && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Drop for AngleBufferBinding {
    fn drop(&mut self) {
        if self.egl_surface != egl::NO_SURFACE && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Debug for AngleSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

pub(crate) enum Synchronization {
    KeyedMutex(ComPtr<IDXGIKeyedMutex>),
    GLFinish,
    None,
}

impl Win32Surface {
    pub(crate) fn angle(&self) -> Result<&AngleWindowBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Win32Binding::Angle(ref binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn angle_mut(&mut self) -> Result<&mut AngleWindowBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Win32Binding::Angle(ref mut binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn present_angle(&self, device: &AngleDevice) -> Result<(), Error> {
        let egl_surface = self.angle()?.egl_surface;
        EGL_FUNCTIONS.with(|egl| unsafe {
            let ok = egl.SwapBuffers(device.egl_display, egl_surface);
            assert_ne!(ok, egl::FALSE);
            Ok(())
        })
    }
}

impl Direct3dSurface {
    pub(crate) fn angle(&self) -> Result<&AngleBufferBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Direct3dBinding::Angle(ref binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn angle_mut(&mut self) -> Result<&mut AngleBufferBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            Direct3dBinding::Angle(ref mut binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }
}

impl AngleSurface {
    /// Returns the DXGI share handle if it has one.
    #[inline]
    pub fn share_handle(&self) -> Option<HANDLE> {
        match self {
            AngleSurface::Window(_) => None,
            AngleSurface::Buffer(surface) => Some(surface.share_handle),
        }
    }

    pub(crate) fn context_id(&self) -> ContextID {
        match self {
            AngleSurface::Window(surface) => surface.context_id,
            AngleSurface::Buffer(surface) => surface.context_id,
        }
    }

    pub(crate) fn egl_surface(&self) -> Result<EGLSurface, Error> {
        match self {
            AngleSurface::Window(surface) => Ok(surface.angle()?.egl_surface),
            AngleSurface::Buffer(surface) => Ok(surface.angle()?.egl_surface),
        }
    }

    pub(crate) fn synchronization(&self) -> Option<&Synchronization> {
        match self {
            AngleSurface::Window(_) => None,
            AngleSurface::Buffer(surface) => Some(&surface.angle().ok()?.synchronization),
        }
    }

    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        match self {
            AngleSurface::Window(surface) => surface.size = size,
            AngleSurface::Buffer(surface) => surface.size = size,
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        match self {
            AngleSurface::Window(surface) => surface.info(),
            AngleSurface::Buffer(surface) => surface.info(),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
