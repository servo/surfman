//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use super::device::Device;
use crate::base::egl::device::EGL_FUNCTIONS;
use crate::context::ContextID;
use crate::egl::types::EGLSurface;
use crate::egl::{self};
use crate::{EglContextDescriptor, Error, SurfaceID, SurfaceInfo};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;
use winapi::shared::dxgi::IDXGIKeyedMutex;
use winapi::um::d3d11;
use winapi::um::winnt::HANDLE;
use wio::com::ComPtr;

/// An implementation of [`crate::Surface`] for Angle on Windows.
pub struct AngleSurface {
    pub(crate) egl_surface: EGLSurface,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) context_descriptor: EglContextDescriptor,
    pub(crate) win32_objects: Win32Objects,
}

/// An implementation of [`crate::SurfaceTexture`] for Angle on Windows.
pub struct AngleSurfaceTexture {
    pub(crate) surface: AngleSurface,
    pub(crate) local_egl_surface: EGLSurface,
    pub(crate) local_keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
    pub(crate) gl_texture: Option<glow::Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for AngleSurface {}

impl Debug for AngleSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Drop for AngleSurface {
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

impl AngleSurface {
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

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: None,
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
