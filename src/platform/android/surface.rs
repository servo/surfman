// surfman/src/platform/android/surface.rs

//! Surface management for Android using the `GraphicBuffer` class and
//! EGL.

use crate::context::ContextID;
use crate::egl::types::{EGLImageKHR, EGLenum, EGLint};
use crate::error::WindowingApiError;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::renderbuffers::Renderbuffers;
use crate::{Error, SurfaceID, egl, gl};
use super::bindings::hardware_buffer::AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM;
use super::bindings::hardware_buffer::AHARDWAREBUFFER_USAGE_CPU_READ_NEVER;
use super::bindings::hardware_buffer::AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER;
use super::bindings::hardware_buffer::AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT;
use super::bindings::hardware_buffer::AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE;
use super::bindings::hardware_buffer::{AHardwareBuffer, AHardwareBuffer_Desc};
use super::bindings::hardware_buffer::{AHardwareBuffer_allocate, AHardwareBuffer_release};
use super::context::{Context, GL_FUNCTIONS};
use super::device::{Device, EGL_EXTENSION_FUNCTIONS};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::ptr;
use std::thread;

const EGL_NATIVE_BUFFER_ANDROID:    EGLenum = 0x3140;

pub struct Surface {
    pub(crate) hardware_buffer: *mut AHardwareBuffer,
    pub(crate) egl_image: EGLImageKHR,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) framebuffer_object: GLuint,
    pub(crate) texture_object: GLuint,
    pub(crate) renderbuffers: Renderbuffers,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) egl_image: EGLImageKHR,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.hardware_buffer as usize)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.framebuffer_object != 0 && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        // https://github.com/fuyufjh/GraphicBuffer
        GL_FUNCTIONS.with(|gl| {
            let hardware_buffer_desc = AHardwareBuffer_Desc {
                format: AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
                width: size.width as u32,
                height: size.height as u32,
                layers: 1,
                rfu0: 0,
                rfu1: 0,
                // FIXME(pcwalton): Why 10?
                stride: 10,
                usage: AHARDWAREBUFFER_USAGE_CPU_READ_NEVER |
                    AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER |
                    AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE |
                    AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT,
            };

            unsafe {
                // Create the Android hardware buffer.
                //
                // FIXME(pcwalton): The Android documentation claims that this function returns
                // `NO_ERROR`, but there is no such symbol in the NDK. I'm going to assume that
                // this means `GL_NO_ERROR`.
                let mut hardware_buffer = ptr::null_mut();
                let err = AHardwareBuffer_allocate(&hardware_buffer_desc, &mut hardware_buffer);
                if err != gl::NO_ERROR as GLint {
                    let windowing_api_error = WindowingApiError::from_gl_error(err);
                    return Err(Error::SurfaceCreationFailed(windowing_api_error));
                }

                // Create an EGL image, and bind it to a texture.
                let egl_image = self.create_egl_image(hardware_buffer);
                let texture_object = self.bind_to_gl_texture(egl_image);

                let mut framebuffer_object = 0;
                gl.GenFramebuffers(1, &mut framebuffer_object);
                gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

                gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        SurfaceTexture::gl_texture_target(),
                                        texture_object,
                                        0);

                let context_descriptor = self.context_descriptor(context);
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);

                let renderbuffers = Renderbuffers::new(&size, &context_attributes);
                renderbuffers.bind_to_current_framebuffer();

                debug_assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                                 gl::FRAMEBUFFER_COMPLETE);

                // Set the viewport so that the application doesn't have to do so explicitly.
                gl.Viewport(0, 0, size.width, size.height);

                Ok(Surface {
                    hardware_buffer,
                    egl_image,
                    size: *size,
                    context_id: context.id,
                    framebuffer_object,
                    texture_object,
                    renderbuffers,
                })
            }
        })
    }

    pub fn create_surface_texture(&self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        unsafe {
            let egl_image = self.create_egl_image(surface.hardware_buffer);
            let texture_object = self.bind_to_gl_texture(egl_image);
            Ok(SurfaceTexture { surface, egl_image, texture_object, phantom: PhantomData })
        }
    }

    unsafe fn create_egl_image(&self, hardware_buffer: *mut AHardwareBuffer) -> EGLImageKHR {
        // Fetch the EGL client buffer.
        let egl_client_buffer =
            (EGL_EXTENSION_FUNCTIONS.GetNativeClientBufferANDROID)(hardware_buffer);
        assert!(!egl_client_buffer.is_null());

        // Create the EGL image.
        let egl_image_attributes = [
            egl::IMAGE_PRESERVED_KHR as EGLint, egl::TRUE as EGLint,
            egl::NONE as EGLint,                0,
        ];
        let egl_image = egl::CreateImageKHR(self.native_display.egl_display(),
                                            egl::NO_CONTEXT,
                                            EGL_NATIVE_BUFFER_ANDROID,
                                            egl_client_buffer,
                                            egl_image_attributes.as_ptr());
        assert_ne!(egl_image, egl::NO_IMAGE_KHR);
        egl_image
    }

    unsafe fn bind_to_gl_texture(&self, egl_image: EGLImageKHR) -> GLuint {
        GL_FUNCTIONS.with(|gl| {
            let mut texture = 0;
            gl.GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl.BindTexture(gl::TEXTURE_2D, texture);
            (EGL_EXTENSION_FUNCTIONS.ImageTargetTexture2DOES)(gl::TEXTURE_2D, egl_image);

            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

            gl.BindTexture(gl::TEXTURE_2D, 0);

            debug_assert_eq!(gl.GetError(), gl::NO_ERROR);
            texture
        })
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // Leak the surface, and return an error.
            surface.framebuffer_object = 0;
            return Err(Error::IncompatibleSurface)
        }

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl.DeleteFramebuffers(1, &surface.framebuffer_object);
                surface.framebuffer_object = 0;
                surface.renderbuffers.destroy();
                gl.DeleteTextures(1, &surface.texture_object);
                surface.texture_object = 0;

                let result = egl::DestroyImageKHR(self.native_display.egl_display(),
                                                  surface.egl_image);
                assert_ne!(result, egl::FALSE);
                surface.egl_image = egl::NO_IMAGE_KHR;

                AHardwareBuffer_release(surface.hardware_buffer);
                surface.hardware_buffer = ptr::null_mut();
            }
        });

        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;

                let result = egl::DestroyImageKHR(self.native_display.egl_display(),
                                                  surface_texture.egl_image);
                assert_ne!(result, egl::FALSE);
                surface_texture.egl_image = egl::NO_IMAGE_KHR;
            }

            Ok(surface_texture.surface)
        })
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        SurfaceID(self.hardware_buffer as usize)
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.texture_object
    }

    #[inline]
    pub fn gl_texture_target() -> GLenum {
        gl::TEXTURE_2D
    }
}
