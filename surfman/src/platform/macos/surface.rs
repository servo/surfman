//! Surface management for macOS.

use crate::context::ContextID;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::renderbuffers::Renderbuffers;
use crate::{Error, SurfaceID, gl};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow};
use io_surface::{kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_RECTANGLE;

pub struct Surface {
    pub(crate) io_surface: IOSurface,
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
                let properties = CFDictionary::from_CFType_pairs(&[
                    (CFString::wrap_under_get_rule(kIOSurfaceWidth),
                    CFNumber::from(size.width).as_CFType()),
                    (CFString::wrap_under_get_rule(kIOSurfaceHeight),
                    CFNumber::from(size.height).as_CFType()),
                    (CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                    CFNumber::from(BYTES_PER_PIXEL).as_CFType()),
                    (CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                    CFNumber::from(size.width * BYTES_PER_PIXEL).as_CFType()),
                ]);

                let io_surface = io_surface::new(&properties);

                let texture_object = self.bind_to_gl_texture(&io_surface, size);

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
                    io_surface,
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
        let texture_object = self.bind_to_gl_texture(&surface.io_surface, &surface.size);
        Ok(SurfaceTexture {
            surface,
            texture_object,
            phantom: PhantomData,
        })
    }

    fn bind_to_gl_texture(&self, io_surface: &IOSurface, size: &Size2D<i32>) -> GLuint {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let mut texture = 0;
                gl.GenTextures(1, &mut texture);
                debug_assert_ne!(texture, 0);

                gl.BindTexture(gl::TEXTURE_RECTANGLE, texture);
                io_surface.bind_to_gl_texture(size.width, size.height, true);

                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                gl::TEXTURE_MAG_FILTER,
                                gl::NEAREST as GLint);
                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                gl::TEXTURE_MIN_FILTER,
                                gl::NEAREST as GLint);
                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                gl::TEXTURE_WRAP_S,
                                gl::CLAMP_TO_EDGE as GLint);
                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                gl::TEXTURE_WRAP_T,
                                gl::CLAMP_TO_EDGE as GLint);

                gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);

                debug_assert_eq!(gl.GetError(), gl::NO_ERROR);

                texture
            }
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
        SurfaceID(self.io_surface.as_concrete_TypeRef() as usize)
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.texture_object
    }
}
