// surfman/src/platform/android/surface.rs

//! Surface management for Android using the `GraphicBuffer` class and
//! EGL.

use crate::context::ContextID;
use crate::egl::types::{EGLClientBuffer, EGLImageKHR, EGLint};
use crate::gl::Gl;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::renderbuffers::Renderbuffers;
use crate::{Error, SurfaceID, egl, gl};
use super::context::{Context, GL_FUNCTIONS};
use super::device::{Device, EGL_EXTENSION_FUNCTIONS};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::ptr;
use std::thread;

// FIXME(pcwalton): Is this right, or should it be `TEXTURE_EXTERNAL_OES`?
const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

pub struct Surface {
    pub(crate) egl_image: EGLImageKHR,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) framebuffer_object: GLuint,
    pub(crate) texture_object: GLuint,
    pub(crate) renderbuffers: Renderbuffers,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
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
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Initialize the texture.
                let mut texture_object = 0;
                gl.GenTextures(1, &mut texture_object);
                gl.BindTexture(gl::TEXTURE_2D, texture_object);
                gl.TexImage2D(gl::TEXTURE_2D,
                              0,
                              gl::RGBA as GLint,
                              size.width,
                              size.height,
                              0,
                              gl::RGBA,
                              gl::UNSIGNED_BYTE,
                              ptr::null());
                self.set_texture_parameters(gl);
                gl.BindTexture(gl::TEXTURE_2D, 0);

                // Create an EGL image, and bind it to a texture.
                let egl_image = self.create_egl_image(context, texture_object);

                let mut framebuffer_object = 0;
                gl.GenFramebuffers(1, &mut framebuffer_object);
                gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

                gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        SURFACE_GL_TEXTURE_TARGET,
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
            let texture_object = self.bind_to_gl_texture(surface.egl_image);
            Ok(SurfaceTexture { surface, texture_object, phantom: PhantomData })
        }
    }

    unsafe fn create_egl_image(&self, context: &Context, texture_object: GLuint) -> EGLImageKHR {
        // Create the EGL image.
        let egl_image_attributes = [
            egl::GL_TEXTURE_LEVEL as EGLint,    0,
            egl::IMAGE_PRESERVED_KHR as EGLint, egl::TRUE as EGLint,
            egl::NONE as EGLint,                0,
        ];
        let egl_image = egl::CreateImageKHR(self.native_display.egl_display(),
                                            context.native_context.egl_context(),
                                            egl::GL_TEXTURE_2D,
                                            texture_object as EGLClientBuffer,
                                            egl_image_attributes.as_ptr());
        assert_ne!(egl_image, egl::NO_IMAGE_KHR);
        egl_image
    }

    unsafe fn set_texture_parameters(&self, gl: &Gl) {
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
    }

    unsafe fn bind_to_gl_texture(&self, egl_image: EGLImageKHR) -> GLuint {
        GL_FUNCTIONS.with(|gl| {
            let mut texture = 0;
            gl.GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl.BindTexture(gl::TEXTURE_2D, texture);
            (EGL_EXTENSION_FUNCTIONS.ImageTargetTexture2DOES)(gl::TEXTURE_2D, egl_image);
            self.set_texture_parameters(gl);
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
            return Err(Error::IncompatibleSurface);
        }

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl.DeleteFramebuffers(1, &surface.framebuffer_object);
                surface.framebuffer_object = 0;
                surface.renderbuffers.destroy();

                let result = egl::DestroyImageKHR(self.native_display.egl_display(),
                                                  surface.egl_image);
                assert_ne!(result, egl::FALSE);
                surface.egl_image = egl::NO_IMAGE_KHR;

                gl.DeleteTextures(1, &surface.texture_object);
                surface.texture_object = 0;
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
            }

            Ok(surface_texture.surface)
        })
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        SurfaceID(self.egl_image as usize)
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.texture_object
    }
}
