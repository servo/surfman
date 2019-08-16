use crate::platform::{NativeSurface, NativeSurfaceTexture};
use crate::{GLContextAttributes, GLFormats, GLVersion, NativeGLContextMethods};
use euclid::default::Size2D;
use gleam::gl::{self, GLuint, Gl, GlType};
use std::mem;

pub enum RenderTarget {
    Unallocated,
    Allocated { size: Size2D<i32>, framebuffer: Framebuffer },
}

pub enum Framebuffer {
    DefaultFramebuffer,
    FramebufferObject {
        framebuffer_name: GLuint,
        color_buffer: NativeSurfaceTexture,
        fbo_renderbuffers: FboRenderbuffers,
    }
}

impl Drop for RenderTarget {
    fn drop(&mut self) {
        match *self {
            RenderTarget::Unallocated => {}
            _ => panic!("Should have destroyed the render target with `destroy()`!"),
        }
    }
}

impl RenderTarget {
    pub(crate) fn new<N>(gl: &dyn Gl,
                         native_context: &mut N,
                         api_type: GlType,
                         api_version: GLVersion,
                         size: &Size2D<i32>,
                         attributes: &GLContextAttributes,
                         formats: &GLFormats)
                         -> Result<RenderTarget, &'static str>
                         where N: NativeGLContextMethods {
        if native_context.uses_default_framebuffer() {
            return Ok(RenderTarget::Allocated {
                size: *size,
                framebuffer: Framebuffer::DefaultFramebuffer,
            });
        }

        native_context.make_current()?;

        let format = formats.to_format().expect("Unexpected format!");
        let surface = NativeSurface::new(gl, api_type, api_version, size, format);
        let color_buffer = NativeSurfaceTexture::new(gl, surface);

        unsafe {
            let framebuffer = gl.gen_framebuffers(1)[0];
            gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);

            gl.framebuffer_texture_2d(gl::FRAMEBUFFER,
                                      gl::COLOR_ATTACHMENT0,
                                      NativeSurfaceTexture::gl_texture_target(),
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
    pub fn color_surface(&self) -> Option<&NativeSurfaceTexture> {
        match *self {
            RenderTarget::Allocated {
                framebuffer: Framebuffer::FramebufferObject { ref color_buffer, .. },
                ..
            } => Some(color_buffer),
            RenderTarget::Allocated { 
        }
    }
    */

    pub(crate) fn swap_color_surface(&mut self, gl: &dyn Gl, surface: NativeSurface)
                                     -> Result<NativeSurface, ()> {
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

        let new_color_buffer = NativeSurfaceTexture::new(gl, surface);
        unsafe {
            gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
            gl.framebuffer_texture_2d(gl::FRAMEBUFFER,
                                      gl::COLOR_ATTACHMENT0,
                                      NativeSurfaceTexture::gl_texture_target(),
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
    pub fn framebuffer_object(&self) -> Option<GLuint> {
        match *self {
            RenderTarget::Unallocated => None,
            RenderTarget::Allocated { framebuffer: Framebuffer::DefaultFramebuffer, .. } => {
                Some(0)
            }
            RenderTarget::Allocated {
                framebuffer: Framebuffer::FramebufferObject { framebuffer_name, .. },
                ..
            } => Some(framebuffer_name),
        }
    }

    #[inline]
    pub fn size(&self) -> Option<Size2D<i32>> {
        match *self {
            RenderTarget::Allocated { size, .. } => Some(size),
            RenderTarget::Unallocated => None,
        }
    }
}

pub enum FboRenderbuffers {
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
    fn new(gl: &dyn Gl, size: &Size2D<i32>, attributes: &GLContextAttributes, formats: &GLFormats)
           -> FboRenderbuffers {
        unsafe {
            if attributes.depth && attributes.stencil && formats.packed_depth_stencil {
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
            if attributes.depth {
                depth_renderbuffer = gl.gen_renderbuffers(1)[0];
                gl.bind_renderbuffer(gl::RENDERBUFFER, depth_renderbuffer);
                gl.renderbuffer_storage(gl::RENDERBUFFER, formats.depth, size.width, size.height);
            }
            if attributes.stencil {
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
