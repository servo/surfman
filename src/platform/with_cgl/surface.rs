//! Surface management for macOS.

use crate::{ContextAttributes, Error, FeatureFlags, GLInfo, SurfaceDescriptor};
use super::context::Context;
use super::device::Device;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl};
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement};
use io_surface::{kIOSurfaceBytesPerRow, kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Clone)]
pub struct Surface {
    pub(crate) io_surface: IOSurface,
    pub(crate) descriptor: Arc<SurfaceDescriptor>,
    pub(crate) destroyed: bool,
}

#[derive(Debug)]
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:?})", self.descriptor)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface_from_descriptor(&self,
                                          _: &dyn Gl,
                                          _: &mut Context,
                                          descriptor: &SurfaceDescriptor)
                                          -> Surface {
        let io_surface = unsafe {
            let props = CFDictionary::from_CFType_pairs(&[
                (CFString::wrap_under_get_rule(kIOSurfaceWidth),
                 CFNumber::from(descriptor.size.width).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceHeight),
                 CFNumber::from(descriptor.size.height).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                 CFNumber::from(BYTES_PER_PIXEL).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                 CFNumber::from(descriptor.size.width * BYTES_PER_PIXEL).as_CFType()),
            ]);
            io_surface::new(&props)
        };

        Surface { io_surface, descriptor: Arc::new(*descriptor), destroyed: false }
    }

    pub fn create_surface_texture(&self, gl: &dyn Gl, _: &mut Context, native_surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_RECTANGLE_ARB, texture);

        let descriptor = native_surface.descriptor();
        let (size, alpha) = (descriptor.size, descriptor.format.has_alpha());
        native_surface.io_surface.bind_to_gl_texture(size.width, size.height, alpha);

        // Low filtering to allow rendering
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_MAG_FILTER,
                           gl::NEAREST as GLint);
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_MIN_FILTER,
                           gl::NEAREST as GLint);

        // TODO(emilio): Check if these two are neccessary, probably not
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_WRAP_S,
                           gl::CLAMP_TO_EDGE as GLint);
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_WRAP_T,
                           gl::CLAMP_TO_EDGE as GLint);

        gl.bind_texture(gl::TEXTURE_RECTANGLE_ARB, 0);

        debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

        Ok(SurfaceTexture { surface: native_surface, gl_texture: texture, phantom: PhantomData })
    }

    pub fn destroy_surface(&self, _: &dyn Gl, _: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        // Nothing to do here.
        surface.destroyed = true;
        Ok(())
    }

    pub fn destroy_surface_texture(&self,
                                   gl: &dyn Gl,
                                   _: &mut Context,
                                   mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        gl.delete_textures(&[surface_texture.gl_texture]);
        surface_texture.gl_texture = 0;
        Ok(surface_texture.surface)
    }
}

impl Surface {
    #[inline]
    pub fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }

    #[inline]
    pub fn gl_texture_target() -> GLenum {
        gl::TEXTURE_RECTANGLE_ARB
    }
}

pub(crate) struct Framebuffer {
    pub(crate) framebuffer_object: GLuint,
    pub(crate) color_surface_texture: SurfaceTexture,
    pub(crate) renderbuffers: Renderbuffers,
}

pub(crate) enum Renderbuffers {
    IndividualDepthStencil {
        depth: GLuint,
        stencil: GLuint,
    },
    CombinedDepthStencil(GLuint),
}

impl Drop for Renderbuffers {
    fn drop(&mut self) {
        match *self {
            Renderbuffers::IndividualDepthStencil { depth: 0, stencil: 0 } |
            Renderbuffers::CombinedDepthStencil(0) => {}
            _ => panic!("Should have destroyed the FBO renderbuffers with `destroy()`!"),
        }
    }
}

impl Renderbuffers {
    pub(crate) fn new(gl: &dyn Gl, size: &Size2D<i32>, info: &GLInfo) -> Renderbuffers {
        if info.attributes.contains(ContextAttributes::DEPTH | ContextAttributes::STENCIL) &&
                info.features.contains(FeatureFlags::SUPPORTS_DEPTH24_STENCIL8) {
            let renderbuffer = gl.gen_renderbuffers(1)[0];
            gl.bind_renderbuffer(gl::RENDERBUFFER, renderbuffer);
            gl.renderbuffer_storage(gl::RENDERBUFFER,
                                    gl::DEPTH24_STENCIL8,
                                    size.width,
                                    size.height);
            gl.bind_renderbuffer(gl::RENDERBUFFER, 0);
            return Renderbuffers::CombinedDepthStencil(renderbuffer);
        }

        let (mut depth_renderbuffer, mut stencil_renderbuffer) = (0, 0);
        if info.attributes.contains(ContextAttributes::DEPTH) {
            depth_renderbuffer = gl.gen_renderbuffers(1)[0];
            gl.bind_renderbuffer(gl::RENDERBUFFER, depth_renderbuffer);
            gl.renderbuffer_storage(gl::RENDERBUFFER,
                                    gl::DEPTH_COMPONENT24,
                                    size.width,
                                    size.height);
        }
        if info.attributes.contains(ContextAttributes::STENCIL) {
            stencil_renderbuffer = gl.gen_renderbuffers(1)[0];
            gl.bind_renderbuffer(gl::RENDERBUFFER, stencil_renderbuffer);
            gl.renderbuffer_storage(gl::RENDERBUFFER, gl::STENCIL_INDEX8, size.width, size.height);
        }
        gl.bind_renderbuffer(gl::RENDERBUFFER, 0);

        Renderbuffers::IndividualDepthStencil {
            depth: depth_renderbuffer,
            stencil: stencil_renderbuffer,
        }
    }

    pub(crate) fn bind_to_current_framebuffer(&self, gl: &dyn Gl) {
        match *self {
            Renderbuffers::CombinedDepthStencil(renderbuffer) => {
                if renderbuffer != 0 {
                    gl.framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                                gl::DEPTH_STENCIL_ATTACHMENT,
                                                gl::RENDERBUFFER,
                                                renderbuffer);
                }
            }
            Renderbuffers::IndividualDepthStencil {
                depth: depth_renderbuffer,
                stencil: stencil_renderbuffer,
            } => {
                if depth_renderbuffer != 0 {
                    gl.framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                                gl::DEPTH_ATTACHMENT,
                                                gl::RENDERBUFFER,
                                                depth_renderbuffer);
                }
                if stencil_renderbuffer != 0 {
                    gl.framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                                gl::STENCIL_ATTACHMENT,
                                                gl::RENDERBUFFER,
                                                stencil_renderbuffer);
                }
            }
        }
    }

    pub(crate) fn destroy(&mut self, gl: &dyn Gl) {
        gl.bind_renderbuffer(gl::RENDERBUFFER, 0);
        match *self {
            Renderbuffers::CombinedDepthStencil(ref mut renderbuffer) => {
                if *renderbuffer != 0 {
                    gl.delete_renderbuffers(&[*renderbuffer]);
                    *renderbuffer = 0;
                }
            }
            Renderbuffers::IndividualDepthStencil {
                depth: ref mut depth_renderbuffer,
                stencil: ref mut stencil_renderbuffer,
            } => {
                if *stencil_renderbuffer != 0 {
                    gl.delete_renderbuffers(&[*stencil_renderbuffer]);
                    *stencil_renderbuffer = 0;
                }
                if *depth_renderbuffer != 0 {
                    gl.delete_renderbuffers(&[*depth_renderbuffer]);
                    *depth_renderbuffer = 0;
                }
            }
        }
    }
}