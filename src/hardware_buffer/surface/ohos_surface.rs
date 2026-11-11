//! Surface management for OpenHarmony OS using EGL.

use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;

use euclid::default::Size2D;
use glow::{HasContext, Texture};
use log::info;

use crate::base::egl::device::EGL_FUNCTIONS;
use crate::base::egl::ffi::EGLImageKHR;
use crate::base::egl::ffi::EGL_EXTENSION_FUNCTIONS;
use crate::base::egl::ffi::EGL_IMAGE_PRESERVED_KHR;
use crate::base::egl::ffi::EGL_NO_IMAGE_KHR;
use crate::context::ContextID;
use crate::egl::types::{EGLSurface, EGLint};
use crate::hardware_buffer::context::HardwareBufferContext;
use crate::hardware_buffer::device::HardwareBufferDevice;
use crate::hardware_buffer::surface::HardwareBufferSurface;
use crate::hardware_buffer::surface::HardwareBufferSurfaceTexture;
use crate::renderbuffers::Renderbuffers;
use crate::{
    egl, gl, gl_utils, Context, Error, Surface, SurfaceAccess, SurfaceTexture, SurfaceType,
};

use super::super::ohos_ffi::{
    eglGetNativeClientBufferANDROID, NativeWindowOperation, OHNativeWindow, OH_NativeBuffer,
    OH_NativeBuffer_Alloc, OH_NativeBuffer_Config, OH_NativeBuffer_Format,
    OH_NativeBuffer_Unreference, OH_NativeBuffer_Usage, OH_NativeWindow_NativeWindowHandleOpt,
    EGL_NATIVE_BUFFER_OHOS,
};
use raw_window_handle::RawWindowHandle;

const SURFACE_GL_TEXTURE_TARGET: u32 = gl::TEXTURE_2D;

/// A surface displayed in an OpenHarmony native window.
pub struct OhosWindowSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) egl_surface: EGLSurface,
    pub(crate) destroyed: bool,
}

/// An offscreen surface backed by an OpenHarmony `OH_NativeBuffer`.
pub struct OhosBufferSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) hardware_buffer: *mut OH_NativeBuffer,
    pub(crate) egl_image: EGLImageKHR,
    pub(crate) framebuffer_object: Option<glow::Framebuffer>,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) renderbuffers: Renderbuffers,
    pub(crate) destroyed: bool,
}

impl Debug for OhosWindowSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "OhosWindowSurface({:x})", self.id().0)
    }
}

impl Debug for OhosBufferSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "OhosBufferSurface({:x})", self.id().0)
    }
}

impl HardwareBufferDevice {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &self,
        context: &Context,
        _: SurfaceAccess,
        surface_type: SurfaceType<'_>,
    ) -> Result<Surface, Error> {
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { window_handle, .. } => unsafe {
                let RawWindowHandle::OhosNdk(handle) = window_handle.as_raw() else {
                    return Err(Error::IncompatibleSurfaceType);
                };
                self.create_window_surface(context, handle.native_window.as_ptr().cast())
            },
        }
    }

    fn create_generic_surface(
        &self,
        context: &Context,
        size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        let concrete_context = context.hardware_buffer()?;
        let _guard = self.temporarily_make_context_current(concrete_context)?;

        let usage = OH_NativeBuffer_Usage::HW_RENDER | OH_NativeBuffer_Usage::HW_TEXTURE;

        let config = OH_NativeBuffer_Config {
            width: size.width,
            height: size.height,
            format: OH_NativeBuffer_Format::RGBA_8888,
            usage: usage,
            stride: 10, // used same magic number as android. I have no idea
        };

        let gl = &concrete_context.gl;
        unsafe {
            let hardware_buffer = OH_NativeBuffer_Alloc(&config as *const _);
            assert!(!hardware_buffer.is_null(), "Failed to create native buffer");

            // Create an EGL image, and bind it to a texture.
            let egl_image = self.create_egl_image(concrete_context, hardware_buffer);

            // Initialize and bind the image to the texture.
            let texture_object =
                crate::base::egl::surface::bind_egl_image_to_gl_texture(gl, egl_image);

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
                gl.check_framebuffer_status(gl::FRAMEBUFFER),
                gl::FRAMEBUFFER_COMPLETE
            );

            Ok(HardwareBufferSurface::Buffer(OhosBufferSurface {
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
    }

    unsafe fn create_window_surface(
        &self,
        context: &Context,
        native_window: *mut OHNativeWindow,
    ) -> Result<Surface, Error> {
        let mut height: i32 = 0;
        let mut width: i32 = 0;
        // Safety: `OH_NativeWindow_NativeWindowHandleOpt` takes two output i32 pointers as
        // variable arguments when called with `GET_BUFFER_GEOMETRY`.
        let result = unsafe {
            OH_NativeWindow_NativeWindowHandleOpt(
                native_window,
                NativeWindowOperation::GET_BUFFER_GEOMETRY,
                &mut height as *mut i32,
                &mut width as *mut i32,
            )
        };

        let context = context.hardware_buffer()?;
        assert_eq!(result, 0, "Failed to determine size of native window");
        EGL_FUNCTIONS.with(|egl| {
            let egl_surface = egl.CreateWindowSurface(
                self.egl_display,
                self.context_to_egl_config(context),
                native_window as *const c_void,
                ptr::null(),
            );
            assert_ne!(egl_surface, egl::NO_SURFACE);

            Ok(HardwareBufferSurface::Window(OhosWindowSurface {
                context_id: context.id,
                size: Size2D::new(width, height),
                egl_surface,
                destroyed: false,
            })
            .into())
        })
    }

    /// Creates a surface texture from an existing generic surface for use with the given context.
    ///
    /// The surface texture is local to the supplied context and takes ownership of the surface.
    /// Destroying the surface texture allows you to retrieve the surface again.
    ///
    /// *The supplied context does not have to be the same context that the surface is associated
    /// with.* This allows you to render to a surface in one context and sample from that surface
    /// in another context.
    ///
    /// Calling this method on a widget surface returns a `WidgetAttached` error.
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        let surface = match surface.try_into()? {
            HardwareBufferSurface::Buffer(surface) => surface,
            surface => return Err((Error::WidgetAttached, surface.into())),
        };
        let context: &mut HardwareBufferContext = match context.try_into() {
            Ok(context) => context,
            Err(error) => return Err((error, HardwareBufferSurface::Buffer(surface).into())),
        };
        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(error) => return Err((error, HardwareBufferSurface::Buffer(surface).into())),
        };

        let gl = &context.gl;
        let local_egl_image = unsafe { self.create_egl_image(context, surface.hardware_buffer) };
        let texture_object =
            unsafe { crate::base::egl::surface::bind_egl_image_to_gl_texture(gl, local_egl_image) };
        Ok(HardwareBufferSurfaceTexture {
            surface,
            local_egl_image,
            texture_object: Some(texture_object),
            phantom: PhantomData,
        }
        .into())
    }

    #[allow(non_snake_case)]
    unsafe fn create_egl_image(
        &self,
        _: &HardwareBufferContext,
        hardware_buffer: *mut OH_NativeBuffer,
    ) -> EGLImageKHR {
        let client_buffer = eglGetNativeClientBufferANDROID(hardware_buffer as *const _);
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
            EGL_NATIVE_BUFFER_OHOS,
            client_buffer.cast_mut().cast(),
            egl_image_attributes.as_ptr(),
        );
        assert_ne!(egl_image, EGL_NO_IMAGE_KHR);
        info!("surfman created an EGL image succesfully!");
        egl_image
    }

    pub(crate) fn destroy_native_buffer_surface(
        &self,
        context: &mut HardwareBufferContext,
        surface: &mut OhosBufferSurface,
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

        let result = unsafe { OH_NativeBuffer_Unreference(surface.hardware_buffer) };
        assert_eq!(result, 0, "OH_NativeBuffer_Unreference failed");
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
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        let context: &mut HardwareBufferContext = match context.try_into() {
            Ok(context) => context,
            Err(error) => return Err((error, surface_texture)),
        };
        let _guard = self.temporarily_make_context_current(context);
        let mut surface_texture: HardwareBufferSurfaceTexture = surface_texture.try_into()?;
        let gl = &context.gl;

        unsafe {
            if let Some(texture) = surface_texture.texture_object.take() {
                gl.delete_texture(texture);
            }

            let egl_display = self.egl_display;
            let result = (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(
                egl_display,
                surface_texture.local_egl_image,
            );
            assert_ne!(result, egl::FALSE);
            surface_texture.local_egl_image = EGL_NO_IMAGE_KHR;
        }

        Ok(HardwareBufferSurface::Buffer(surface_texture.surface).into())
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface) -> Result<SurfaceDataGuard<'s>, Error> {
        error!("lock_surface_data not implemented yet for OHOS");
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
