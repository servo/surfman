// surfman/surfman/src/platform/macos/cgl/surface.rs
//
//! Surface management for macOS.

use crate::context::ContextID;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl_utils;
use crate::platform::macos::system::surface::Surface as SystemSurface;
use crate::renderbuffers::Renderbuffers;
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType, gl};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

use core_foundation::base::TCFType;
use euclid::default::Size2D;
use io_surface::{self, IOSurface};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;

pub use crate::platform::macos::system::surface::NativeWidget;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_RECTANGLE;

pub struct Surface {
    pub(crate) system_surface: SystemSurface,
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

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl Device {
    pub fn create_surface(&mut self,
                          context: &Context,
                          access: SurfaceAccess,
                          surface_type: SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        let system_surface = self.0.create_surface(access, surface_type)?;

        let _guard = self.temporarily_make_context_current(context);
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let texture_object = self.bind_to_gl_texture(&system_surface.io_surface,
                                                             &system_surface.size);

                let mut framebuffer_object = 0;
                gl.GenFramebuffers(1, &mut framebuffer_object);
                let _guard = self.temporarily_bind_framebuffer(framebuffer_object);

                gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        SURFACE_GL_TEXTURE_TARGET,
                                        texture_object,
                                        0);

                let context_descriptor = self.context_descriptor(context);
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);

                let renderbuffers = Renderbuffers::new(gl,
                                                       &system_surface.size,
                                                       &context_attributes);
                renderbuffers.bind_to_current_framebuffer(gl);

                debug_assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                                 gl::FRAMEBUFFER_COMPLETE);

                Ok(Surface {
                    system_surface,
                    context_id: context.id,
                    framebuffer_object,
                    texture_object,
                    renderbuffers,
                })
            }
        })
    }

    pub fn create_surface_texture(&self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, (Error, Surface)> {
        if surface.system_surface.view_info.is_some() {
            return Err((Error::WidgetAttached, surface));
        }

        let texture_object = self.bind_to_gl_texture(&surface.system_surface.io_surface,    
                                                     &surface.system_surface.size);
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

    pub fn destroy_surface(&self, context: &mut Context, surface: &mut Surface)
                           -> Result<(), Error> {
        GL_FUNCTIONS.with(|gl| {
            if context.id != surface.context_id {
                return Err(Error::IncompatibleSurface);
            }

            unsafe {
                gl_utils::destroy_framebuffer(gl, surface.framebuffer_object);
                surface.framebuffer_object = 0;

                surface.renderbuffers.destroy(gl);
                gl.DeleteTextures(1, &surface.texture_object);
                surface.texture_object = 0;
            }

            self.0.destroy_surface(&mut surface.system_surface)
        })
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, (Error, SurfaceTexture)> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;
            }

            Ok(surface_texture.surface)
        })
    }

    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.texture_object
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        self.0.present_surface(&mut surface.system_surface)?;

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let size = surface.system_surface.size;
                gl.BindTexture(gl::TEXTURE_RECTANGLE, surface.texture_object);
                surface.system_surface
                       .io_surface
                       .bind_to_gl_texture(size.width, size.height, true);
                gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);
            }

            Ok(())
        })
    }

    fn temporarily_bind_framebuffer(&self, new_framebuffer: GLuint) -> FramebufferGuard {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let (mut current_draw_framebuffer, mut current_read_framebuffer) = (0, 0);
                gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut current_draw_framebuffer);
                gl.GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut current_read_framebuffer);
                gl.BindFramebuffer(gl::FRAMEBUFFER, new_framebuffer);
                FramebufferGuard {
                    draw: current_draw_framebuffer as GLuint,
                    read: current_read_framebuffer as GLuint,
                }
            }
        })
    }

    #[inline]
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        let system_surface_info = self.0.surface_info(&surface.system_surface);
        SurfaceInfo {
            size: system_surface_info.size,
            id: system_surface_info.id,
            context_id: surface.context_id,
            framebuffer_object: surface.framebuffer_object,
        }
    }
}

impl Surface {
    #[inline]
    fn id(&self) -> SurfaceID {
        SurfaceID(self.system_surface.io_surface.as_concrete_TypeRef() as usize)
    }
}

#[must_use]
struct FramebufferGuard {
    draw: GLuint,
    read: GLuint,
}

impl Drop for FramebufferGuard {
    fn drop(&mut self) {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.BindFramebuffer(gl::READ_FRAMEBUFFER, self.read);
                gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.draw);
            }
        })
    }
}
