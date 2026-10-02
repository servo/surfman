//! Surface management for Android and OpenHarmony using the `GraphicBuffer` class and EGL.

use crate::base::egl::ffi::EGLImageKHR;
use crate::context::ContextID;
use crate::hardware_buffer::context::HardwareBufferContext;
use crate::{Context, Device, Error, Surface, SurfaceID, SurfaceInfo};

use crate::base::egl::device::EGL_FUNCTIONS;
use euclid::default::Size2D;
use glow::Texture;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;

#[cfg(android_platform)]
mod android_surface;

#[cfg(android_platform)]
pub use android_surface::*;

#[cfg(ohos_platform)]
mod ohos_surface;

#[cfg(ohos_platform)]
pub use ohos_surface::*;

/// An implementation of [`crate::Surface`] for Android and OHOS platforms.
pub struct HardwareBufferSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) objects: SurfaceObjects,
    pub(crate) destroyed: bool,
}

/// An implementation of [`crate::SurfaceTexture`] for Android and OHOS platforms.
pub struct HardwareBufferSurfaceTexture {
    pub(crate) surface: HardwareBufferSurface,
    pub(crate) local_egl_image: EGLImageKHR,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for HardwareBufferSurface {}

impl Debug for HardwareBufferSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Drop for HardwareBufferSurface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl HardwareBufferSurface {
    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        self.size = size;
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: match self.objects {
                SurfaceObjects::HardwareBuffer {
                    framebuffer_object, ..
                } => framebuffer_object,
                SurfaceObjects::Window { .. } => None,
            },
        }
    }

    pub(crate) fn id(&self) -> SurfaceID {
        match self.objects {
            SurfaceObjects::HardwareBuffer { egl_image, .. } => SurfaceID(egl_image as usize),
            SurfaceObjects::Window { egl_surface } => SurfaceID(egl_surface as usize),
        }
    }
}

impl Debug for HardwareBufferSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl Device {
    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        self.present_surface_inner(context.hardware_buffer()?, surface.hardware_buffer()?)
    }

    pub(crate) fn present_surface_inner(
        &self,
        context: &HardwareBufferContext,
        surface: &HardwareBufferSurface,
    ) -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| unsafe {
            match surface.objects {
                SurfaceObjects::Window { egl_surface } => {
                    egl.SwapBuffers(self.egl_display, egl_surface);
                    Ok(())
                }
                _ => Err(Error::NoWidgetAttached),
            }
        })
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        _context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        let surface: &mut HardwareBufferSurface = surface.try_into()?;
        surface.resize(size);
        Ok(())
    }
}
