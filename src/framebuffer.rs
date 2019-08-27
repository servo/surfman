//! Encapsulates any OpenGL resources needed for the target framebuffer.
//!
//! Some backends, such as iOS and macOS, don't have a default framebuffer and therefore need some
//! OpenGL objects to be kept around. This object encapsulates them.

use crate::platform::{Display, NativeGLContext, Surface, SurfaceTexture};
use crate::{GLContextAttributes, GLFormats, GLVersion};
use euclid::default::Size2D;
use gleam::gl::{self, GLuint, Gl, GlType};
use std::mem;

pub(crate) struct Framebuffer {
    framebuffer_name: GLuint,
    color_surface: SurfaceTexture,
    fbo_renderbuffers: FboRenderbuffers,
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        if self.framebuffer_name != 0 {
            panic!("Should have destroyed the framebuffers with `destroy()`!");
        }
    }
}

impl Framebuffer {
    pub(crate) fn new(gl: &dyn Gl,
                      descriptor: &SurfaceDescriptor,
                      attributes: &GLContextAttributes,
                      formats: &GLFormats)
                      -> Result<RenderTarget, &'static str> {
        if native_context.uses_default_framebuffer() {
            return Ok(RenderTarget::Allocated {
                size: *size,
                framebuffer: Framebuffer::DefaultFramebuffer,
            });
        }

        native_context.make_current()?;

        let format = formats.to_format().expect("Unexpected format!");
        let surface = display.create_surface_from_descriptor(gl, descriptor);
        let color_buffer = display.create_native_surface_texture(gl, surface);

        unsafe {
            let framebuffer = gl.gen_framebuffers(1)[0];
            gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);

            gl.framebuffer_texture_2d(gl::FRAMEBUFFER,
                                      gl::COLOR_ATTACHMENT0,
                                      SurfaceTexture::gl_texture_target(),
                                      color_buffer.gl_texture(),
                                      0);

            let fbo_renderbuffers = FboRenderbuffers::new(gl, size, attributes, formats);
            fbo_renderbuffers.bind_to_current_framebuffer(gl);

            debug_assert_eq!(gl.check_frame_buffer_status(gl::FRAMEBUFFER),
                             gl::FRAMEBUFFER_COMPLETE);

            Ok(RenderTarget::Allocated {
                size: *size,
                framebuffer: Framebuffer::FramebufferObject {
                    framebuffer_name: framebuffer,
                    color_buffer,
                    fbo_renderbuffers,
                },
            })
        }
    }

    /*
    #[inline]
    pub fn color_surface(&self) -> Option<&SurfaceTexture> {
        match *self {
            RenderTarget::Allocated {
                framebuffer: Framebuffer::FramebufferObject { ref color_buffer, .. },
                ..
            } => Some(color_buffer),
            RenderTarget::Allocated { 
        }
    }
    */

    pub(crate) fn swap_color_surface(&mut self, gl: &dyn Gl, surface: Surface)
                                     -> Result<Surface, ()> {
        let (framebuffer, old_color_buffer) = match *self {
            RenderTarget::Allocated {
                framebuffer: Framebuffer::FramebufferObject {
                    framebuffer_name,
                    ref mut color_buffer,
                    fbo_renderbuffers: _,
                },
                ..
            } => (framebuffer_name, color_buffer),
            RenderTarget::Allocated { framebuffer: Framebuffer::DefaultFramebuffer, .. } |
            RenderTarget::Unallocated => return Err(()),
        };

        let new_color_buffer = SurfaceTexture::new(gl, surface);
        unsafe {
            gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
            gl.framebuffer_texture_2d(gl::FRAMEBUFFER,
                                      gl::COLOR_ATTACHMENT0,
                                      SurfaceTexture::gl_texture_target(),
                                      new_color_buffer.gl_texture(),
                                      0);
        }

        Ok(mem::replace(old_color_buffer, new_color_buffer).into_surface(gl))
    }

    #[inline]
    pub(crate) fn take(&mut self) -> RenderTarget {
        mem::replace(self, RenderTarget::Unallocated)
    }

    pub(crate) fn destroy(&mut self, gl: &dyn Gl) {
        let mut this = self.take();
        match this {
            RenderTarget::Allocated {
                framebuffer: Framebuffer::FramebufferObject {
                    ref mut framebuffer_name,
                    ref mut color_buffer,
                    ref mut fbo_renderbuffers,
                },
                ..
            } => {
                fbo_renderbuffers.destroy(gl);
                color_buffer.destroy(gl);
                gl.delete_framebuffers(&[*framebuffer_name]);
                *framebuffer_name = 0;
            }
            RenderTarget::Allocated { framebuffer: Framebuffer::DefaultFramebuffer, .. } |
            RenderTarget::Unallocated => {}
        }
        mem::forget(this);
    }

    #[inline]
    pub fn framebuffer_name(&self) -> GLuint {
        self.framebuffer_name
    }
}

pub(crate) enum FboRenderbuffers {
    IndividualDepthStencil {
        depth: GLuint,
        stencil: GLuint,
    },
    CombinedDepthStencil(GLuint),
}

impl Drop for FboRenderbuffers {
    fn drop(&mut self) {
        match *self {
            FboRenderbuffers::IndividualDepthStencil { depth: 0, stencil: 0 } |
            FboRenderbuffers::CombinedDepthStencil(0) => {}
            _ => panic!("Should have destroyed the FBO renderbuffers with `destroy()`!"),
        }
    }
}

impl FboRenderbuffers {
    fn new(gl: &dyn Gl, size: &Size2D<i32>, info: &GLInfo) -> FboRenderbuffers {
        unsafe {
            if info.attributes.contains(ContextAttributes::DEPTH | ContextAttributes::STENCIL) &&
                    info.features.contains(FeatureFlags::SUPPORTS_DEPTH24_STENCIL8) {
                let renderbuffer = gl.gen_renderbuffers(1)[0];
                gl.bind_renderbuffer(gl::RENDERBUFFER, renderbuffer);
                gl.renderbuffer_storage(gl::RENDERBUFFER,
                                        gl::DEPTH24_STENCIL8,
                                        size.width,
                                        size.height);
                gl.bind_renderbuffer(gl::RENDERBUFFER, 0);
                return FboRenderbuffers::CombinedDepthStencil(renderbuffer);
            }

            let (mut depth_renderbuffer, mut stencil_renderbuffer) = (0, 0);
            if info.attributes.contains(ContextAttributes::DEPTH) {
                depth_renderbuffer = gl.gen_renderbuffers(1)[0];
                gl.bind_renderbuffer(gl::RENDERBUFFER, depth_renderbuffer);
                gl.renderbuffer_storage(gl::RENDERBUFFER, formats.depth, size.width, size.height);
            }
            if info.attributes.contains(ContextAttributes::STENCIL) {
                stencil_renderbuffer = gl.gen_renderbuffers(1)[0];
                gl.bind_renderbuffer(gl::RENDERBUFFER, stencil_renderbuffer);
                gl.renderbuffer_storage(gl::RENDERBUFFER,
                                        formats.stencil,
                                        size.width,
                                        size.height);
            }
            gl.bind_renderbuffer(gl::RENDERBUFFER, 0);

            FboRenderbuffers::IndividualDepthStencil {
                depth: depth_renderbuffer,
                stencil: stencil_renderbuffer,
            }
        }
    }

    fn bind_to_current_framebuffer(&self, gl: &dyn Gl) {
        unsafe {
            match *self {
                FboRenderbuffers::CombinedDepthStencil(renderbuffer) => {
                    if renderbuffer != 0 {
                        gl.framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                                    gl::DEPTH_STENCIL_ATTACHMENT,
                                                    gl::RENDERBUFFER,
                                                    renderbuffer);
                    }
                }
                FboRenderbuffers::IndividualDepthStencil {
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
    }

    fn destroy(&mut self, gl: &dyn Gl) {
        unsafe {
            gl.bind_renderbuffer(gl::RENDERBUFFER, 0);
            match *self {
                FboRenderbuffers::CombinedDepthStencil(ref mut renderbuffer) => {
                    if *renderbuffer != 0 {
                        gl.delete_renderbuffers(&[*renderbuffer]);
                        *renderbuffer = 0;
                    }
                }
                FboRenderbuffers::IndividualDepthStencil {
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
}
