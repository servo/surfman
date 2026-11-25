//! Surface management for Android and OpenHarmony using `AHardwareBuffer` / `OH_NativeBuffer` and EGL.

use crate::base::egl::device::EGL_FUNCTIONS;
use crate::base::egl::ffi::{
    EGLImageKHR, EGL_EXTENSION_FUNCTIONS, EGL_IMAGE_PRESERVED_KHR, EGL_NO_IMAGE_KHR,
};
use crate::base::egl::surface::bind_egl_image_to_gl_texture;
use crate::context::ContextID;
use crate::egl::types::{EGLSurface, EGLint};
use crate::hardware_buffer::context::HardwareBufferContext;
use crate::hardware_buffer::device::HardwareBufferDevice;
use crate::renderbuffers::Renderbuffers;
use crate::{
    egl, gl, gl_utils, Context, Error, OffscreenSurface, Surface, SurfaceAccess, SurfaceID,
    SurfaceInfo, SurfaceTexture, WindowSurface,
};

use euclid::default::Size2D;
use glow::{HasContext, Texture};
use raw_window_handle::WindowHandle;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use std::thread;

#[cfg(android_platform)]
mod android_surface;

#[cfg(android_platform)]
pub use android_surface::*;

#[cfg(ohos_platform)]
mod ohos_surface;

#[cfg(ohos_platform)]
pub use ohos_surface::*;

#[cfg(android_platform)]
use android_surface as platform;
#[cfg(android_platform)]
pub(crate) use android_surface::{
    AndroidBufferSurface as PlatformBufferSurface, AndroidWindowSurface as PlatformWindowSurface,
};
#[cfg(ohos_platform)]
use ohos_surface as platform;
#[cfg(ohos_platform)]
pub(crate) use ohos_surface::{
    OhosBufferSurface as PlatformBufferSurface, OhosWindowSurface as PlatformWindowSurface,
};

const SURFACE_GL_TEXTURE_TARGET: u32 = gl::TEXTURE_2D;

/// A surface displayed in a native window, shared by the Android and OHOS window surfaces.
pub(crate) struct NativeWindowSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) egl_surface: EGLSurface,
    pub(crate) destroyed: bool,
}

/// An offscreen surface backed by a native buffer, shared by the Android and OHOS buffer surfaces.
pub(crate) struct NativeBufferSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) hardware_buffer: *mut platform::NativeBuffer,
    pub(crate) egl_image: EGLImageKHR,
    pub(crate) framebuffer_object: Option<glow::Framebuffer>,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) renderbuffers: Renderbuffers,
    pub(crate) destroyed: bool,
}

/// A surface bound to a hardware buffer context.
pub(crate) enum HardwareBufferSurface {
    /// A surface displayed in a native window.
    Window(PlatformWindowSurface),
    /// An offscreen surface backed by a hardware buffer.
    Buffer(PlatformBufferSurface),
}

/// An implementation of [`crate::SurfaceTexture`] for Android and OHOS platforms.
pub struct HardwareBufferSurfaceTexture {
    pub(crate) surface: PlatformBufferSurface,
    pub(crate) local_egl_image: EGLImageKHR,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for NativeWindowSurface {}
unsafe impl Send for NativeBufferSurface {}

impl TryFrom<Surface> for HardwareBufferSurface {
    type Error = (Error, Surface);
    fn try_from(surface: Surface) -> Result<HardwareBufferSurface, Self::Error> {
        match surface {
            Surface::Window(surface) => surface
                .try_into()
                .map(HardwareBufferSurface::Window)
                .map_err(|(error, surface)| (error, Surface::Window(surface))),
            Surface::Offscreen(surface) => surface
                .try_into()
                .map(HardwareBufferSurface::Buffer)
                .map_err(|(error, surface)| (error, Surface::Offscreen(surface))),
        }
    }
}

impl From<HardwareBufferSurface> for Surface {
    fn from(surface: HardwareBufferSurface) -> Surface {
        match surface {
            HardwareBufferSurface::Window(surface) => Surface::Window(surface.into()),
            HardwareBufferSurface::Buffer(surface) => Surface::Offscreen(surface.into()),
        }
    }
}

impl Drop for NativeWindowSurface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_window_surface()`!")
        }
    }
}

impl Drop for NativeBufferSurface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_offscreen_surface()`!")
        }
    }
}

impl NativeWindowSurface {
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

impl NativeBufferSurface {
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
            HardwareBufferSurface::Window(surface) => surface.0.context_id,
            HardwareBufferSurface::Buffer(surface) => surface.0.context_id,
        }
    }

    pub(crate) fn resize(&mut self, size: Size2D<i32>) {
        match self {
            HardwareBufferSurface::Window(surface) => surface.0.size = size,
            HardwareBufferSurface::Buffer(surface) => surface.0.size = size,
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        match self {
            HardwareBufferSurface::Window(surface) => surface.0.info(),
            HardwareBufferSurface::Buffer(surface) => surface.0.info(),
        }
    }
}

impl Debug for HardwareBufferSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl HardwareBufferDevice {
    /// See [`crate::Device::create_window_surface`].
    pub fn create_window_surface(
        &self,
        context: &Context,
        _: SurfaceAccess,
        window_handle: WindowHandle<'_>,
        _: Size2D<i32>,
    ) -> Result<WindowSurface, Error> {
        let native_window = platform::native_window(window_handle.as_raw())?;
        unsafe { self.create_native_window_surface(context, native_window) }
    }

    /// See [`crate::Device::create_offscreen_surface`].
    pub fn create_offscreen_surface(
        &self,
        context: &Context,
        _: SurfaceAccess,
        size: Size2D<i32>,
    ) -> Result<OffscreenSurface, Error> {
        self.create_generic_surface(context, &size)
    }

    fn create_generic_surface(
        &self,
        context: &Context,
        size: &Size2D<i32>,
    ) -> Result<OffscreenSurface, Error> {
        let concrete_context = context.hardware_buffer()?;
        let _guard = self.temporarily_make_context_current(concrete_context)?;
        let gl = &concrete_context.gl;

        let hardware_buffer = platform::allocate_native_buffer(size)?;

        // Create an EGL image, and bind it to a texture.
        let egl_image = unsafe { self.create_egl_image(concrete_context, hardware_buffer) };

        // Initialize and bind the image to the texture.
        let texture_object = unsafe { bind_egl_image_to_gl_texture(gl, egl_image) };

        // Create the framebuffer, and bind the texture to it.
        let framebuffer_object = gl_utils::create_and_bind_framebuffer(
            gl,
            SURFACE_GL_TEXTURE_TARGET,
            Some(texture_object),
        );

        // Bind renderbuffers as appropriate.
        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);
        let renderbuffers = Renderbuffers::new(gl, size, &context_attributes);
        renderbuffers.bind_to_current_framebuffer(gl);

        debug_assert_eq!(
            unsafe { gl.check_framebuffer_status(gl::FRAMEBUFFER) },
            gl::FRAMEBUFFER_COMPLETE
        );

        Ok(PlatformBufferSurface(NativeBufferSurface {
            size: *size,
            context_id: concrete_context.id,
            hardware_buffer,
            egl_image,
            framebuffer_object: Some(framebuffer_object),
            texture_object: Some(texture_object),
            renderbuffers,
            destroyed: false,
        })
        .into())
    }

    unsafe fn create_native_window_surface(
        &self,
        context: &Context,
        native_window: *mut platform::NativeWindow,
    ) -> Result<WindowSurface, Error> {
        let size = unsafe { platform::native_window_size(native_window) };
        let context = context.hardware_buffer()?;

        let egl_surface = EGL_FUNCTIONS.with(|egl| unsafe {
            egl.CreateWindowSurface(
                self.egl_display,
                self.context_to_egl_config(context),
                native_window as *const c_void,
                ptr::null(),
            )
        });
        assert_ne!(egl_surface, egl::NO_SURFACE);

        Ok(PlatformWindowSurface(NativeWindowSurface {
            context_id: context.id,
            size,
            egl_surface,
            destroyed: false,
        })
        .into())
    }

    /// Creates a surface texture from an existing generic surface for use with the given context.
    ///
    /// The surface texture is local to the supplied context and takes ownership of the surface.
    /// Destroying the surface texture allows you to retrieve the surface again.
    ///
    /// *The supplied context does not have to be the same context that the surface is associated
    /// with.* This allows you to render to a surface in one context and sample from that surface
    /// in another context.
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: OffscreenSurface,
    ) -> Result<SurfaceTexture, (Error, OffscreenSurface)> {
        let surface: PlatformBufferSurface = surface.try_into()?;
        let context: &mut HardwareBufferContext = match context.try_into() {
            Ok(context) => context,
            Err(error) => return Err((error, surface.into())),
        };
        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(error) => return Err((error, surface.into())),
        };

        let gl = &context.gl;
        let local_egl_image = unsafe { self.create_egl_image(context, surface.0.hardware_buffer) };
        let texture_object = unsafe { bind_egl_image_to_gl_texture(gl, local_egl_image) };
        Ok(HardwareBufferSurfaceTexture {
            surface,
            local_egl_image,
            texture_object: Some(texture_object),
            phantom: PhantomData,
        }
        .into())
    }

    unsafe fn create_egl_image(
        &self,
        _: &HardwareBufferContext,
        hardware_buffer: *mut platform::NativeBuffer,
    ) -> EGLImageKHR {
        let (client_buffer, target) = unsafe { platform::native_client_buffer(hardware_buffer) };
        assert!(!client_buffer.is_null());

        // Create the EGL image.
        let egl_image_attributes = [
            EGL_IMAGE_PRESERVED_KHR as EGLint,
            egl::TRUE as EGLint,
            egl::NONE as EGLint,
            0,
        ];
        let egl_image = (EGL_EXTENSION_FUNCTIONS.CreateImageKHR)(
            self.egl_display,
            egl::NO_CONTEXT,
            target,
            client_buffer,
            egl_image_attributes.as_ptr(),
        );
        assert_ne!(egl_image, EGL_NO_IMAGE_KHR);
        egl_image
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(
        &self,
        context: &Context,
        surface: &mut WindowSurface,
    ) -> Result<(), Error> {
        let surface: &mut PlatformWindowSurface = surface.try_into()?;
        self.present_window_surface(context.hardware_buffer()?, &surface.0)
    }

    pub(crate) fn present_window_surface(
        &self,
        context: &HardwareBufferContext,
        surface: &NativeWindowSurface,
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
        surface: &mut WindowSurface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        let surface: &mut PlatformWindowSurface = surface.try_into()?;
        surface.0.size = size;
        Ok(())
    }

    pub(crate) fn destroy_native_window_surface(
        &self,
        context: &mut HardwareBufferContext,
        surface: &mut NativeWindowSurface,
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

    pub(crate) fn destroy_native_buffer_surface(
        &self,
        context: &mut HardwareBufferContext,
        surface: &mut NativeBufferSurface,
    ) -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        let gl = &context.gl;
        unsafe { gl.bind_framebuffer(gl::FRAMEBUFFER, None) };
        if let Some(framebuffer) = surface.framebuffer_object.take() {
            unsafe { gl.delete_framebuffer(framebuffer) };
        }

        surface.renderbuffers.destroy(gl);

        if let Some(texture) = surface.texture_object.take() {
            unsafe { gl.delete_texture(texture) };
        }

        let result = (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(self.egl_display, surface.egl_image);
        assert_ne!(result, egl::FALSE);
        surface.egl_image = EGL_NO_IMAGE_KHR;

        unsafe { platform::release_native_buffer(surface.hardware_buffer) };
        surface.hardware_buffer = ptr::null_mut();

        surface.destroyed = true;
        Ok(())
    }

    /// Destroys a surface texture and returns the underlying surface.
    ///
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    ///
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context,
        surface_texture: SurfaceTexture,
    ) -> Result<OffscreenSurface, (Error, SurfaceTexture)> {
        let context: &mut HardwareBufferContext = match context.try_into() {
            Ok(context) => context,
            Err(error) => return Err((error, surface_texture)),
        };
        let _guard = self.temporarily_make_context_current(context);
        let mut surface_texture: HardwareBufferSurfaceTexture = surface_texture.try_into()?;
        let gl = &context.gl;

        if let Some(texture) = surface_texture.texture_object.take() {
            unsafe { gl.delete_texture(texture) };
        }

        let result = (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(
            self.egl_display,
            surface_texture.local_egl_image,
        );
        assert_ne!(result, egl::FALSE);
        surface_texture.local_egl_image = EGL_NO_IMAGE_KHR;

        Ok(surface_texture.surface.into())
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface) -> Result<SurfaceDataGuard<'s>, Error> {
        // TODO(pcwalton)
        Err(Error::Unimplemented)
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> u32 {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> Option<Texture> {
        surface_texture.hardware_buffer().ok()?.texture_object
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
