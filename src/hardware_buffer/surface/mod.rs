//! Surface management for Android and OpenHarmony using the `GraphicBuffer` class and EGL.

use crate::base::egl::ffi::EGLImageKHR;
use crate::context::ContextID;
use crate::egl::types::EGLSurface;
use crate::hardware_buffer::context::HardwareBufferContext;
use crate::hardware_buffer::device::HardwareBufferDevice;
use crate::renderbuffers::Renderbuffers;
use crate::{egl, Context, Error, Surface, SurfaceID, SurfaceInfo};

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

/// A surface displayed in a native window on Android and OHOS platforms.
pub struct HardwareBufferWindowSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) egl_surface: EGLSurface,
    pub(crate) destroyed: bool,
}

/// An offscreen surface backed by a hardware buffer on Android and OHOS platforms.
pub struct HardwareBufferBufferSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) hardware_buffer: *mut NativeBuffer,
    pub(crate) egl_image: EGLImageKHR,
    pub(crate) framebuffer_object: Option<glow::Framebuffer>,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) renderbuffers: Renderbuffers,
    pub(crate) destroyed: bool,
}

/// An implementation of [`crate::Surface`] for Android and OHOS platforms.
#[derive(Debug)]
pub enum HardwareBufferSurface {
    /// A surface displayed in a native window.
    Window(HardwareBufferWindowSurface),
    /// An offscreen surface backed by a hardware buffer.
    Buffer(HardwareBufferBufferSurface),
}

/// An implementation of [`crate::SurfaceTexture`] for Android and OHOS platforms.
pub struct HardwareBufferSurfaceTexture {
    pub(crate) surface: HardwareBufferBufferSurface,
    pub(crate) local_egl_image: EGLImageKHR,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for HardwareBufferWindowSurface {}
unsafe impl Send for HardwareBufferBufferSurface {}

impl Debug for HardwareBufferWindowSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "HardwareBufferWindowSurface({:x})", self.id().0)
    }
}

impl Debug for HardwareBufferBufferSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "HardwareBufferBufferSurface({:x})", self.id().0)
    }
}

impl Drop for HardwareBufferWindowSurface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Drop for HardwareBufferBufferSurface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl HardwareBufferWindowSurface {
    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: None,
        }
    }

    pub(crate) fn id(&self) -> SurfaceID {
        SurfaceID(self.egl_surface as usize)
    }
}

impl HardwareBufferBufferSurface {
    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: self.framebuffer_object,
        }
    }

    pub(crate) fn id(&self) -> SurfaceID {
        SurfaceID(self.egl_image as usize)
    }
}

impl HardwareBufferSurface {
    pub(crate) fn context_id(&self) -> ContextID {
        match self {
            HardwareBufferSurface::Window(surface) => surface.context_id,
            HardwareBufferSurface::Buffer(surface) => surface.context_id,
        }
    }

    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        match self {
            HardwareBufferSurface::Window(surface) => surface.size = size,
            HardwareBufferSurface::Buffer(surface) => surface.size = size,
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        match self {
            HardwareBufferSurface::Window(surface) => surface.info(),
            HardwareBufferSurface::Buffer(surface) => surface.info(),
        }
    }
}

impl Debug for HardwareBufferSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl HardwareBufferDevice {
    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        let context = context.hardware_buffer()?;
        match surface.hardware_buffer()? {
            HardwareBufferSurface::Window(surface) => self.present_window_surface(context, surface),
            HardwareBufferSurface::Buffer(_) => Err(Error::NoWidgetAttached),
        }
    }

    pub(crate) fn present_window_surface(
        &self,
        context: &HardwareBufferContext,
        surface: &HardwareBufferWindowSurface,
    ) -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| unsafe {
            egl.SwapBuffers(self.egl_display, surface.egl_surface);
        });
        Ok(())
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

    pub(crate) fn destroy_native_window_surface(
        &self,
        context: &mut HardwareBufferContext,
        surface: &mut HardwareBufferWindowSurface,
    ) -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| unsafe {
            egl.DestroySurface(self.egl_display, surface.egl_surface);
        });
        surface.egl_surface = egl::NO_SURFACE;

        surface.destroyed = true;
        Ok(())
    }
}
