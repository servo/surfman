//! Wrapper for GL-renderable pixmaps on X11.

use crate::{ContextAttributeFlags, ContextAttributes, Error, FeatureFlags, GLInfo, SurfaceId};
use super::context::{Context, ContextID};
use super::device::Device;

use euclid::default::Size2D;
use gl;
use gl::types::{GLenum, GLint, GLuint};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;
use x11::xlib;

pub struct Surface {
    pub(crate) glx_pixmap: GLXPixmap,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.glx_pixmap as usize)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.glx_pixmap != xlib::None && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        let display = self.native_display.display();

        let context_descriptor = self.context_descriptor(context);
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(context_descriptor);

        unsafe {
            let mut glx_visual_id = xlib::None;
            let result = glx::GetFBConfigAttrib(display,
                                                glx_fb_config,
                                                GLX_VISUAL_ID,
                                                &mut glx_visual_id);
            if result != xlib::Success {
                return Err(Error::SurfaceCreationFailed());
            }
        }

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
            gl::GenFramebuffers(1, &mut framebuffer_object);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     SurfaceTexture::gl_texture_target(),
                                     texture_object,
                                     0);

            let context_descriptor = self.context_descriptor(context);
            let context_attributes = self.context_descriptor_attributes(&context_descriptor);

            let renderbuffers = Renderbuffers::new(&size, &context_attributes, &context.gl_info);
            renderbuffers.bind_to_current_framebuffer();

            debug_assert_eq!(gl::CheckFramebufferStatus(gl::FRAMEBUFFER),
                             gl::FRAMEBUFFER_COMPLETE);

            // Set the viewport so that the application doesn't have to do so explicitly.
            gl::Viewport(0, 0, size.width, size.height);

            Ok(Surface {
                io_surface,
                size: *size,
                context_id: context.id,
                framebuffer_object,
                texture_object,
                renderbuffers,
            })
        }
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
        unsafe {
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl::BindTexture(gl::TEXTURE_RECTANGLE, texture);
            io_surface.bind_to_gl_texture(size.width, size.height, true);

            gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE,
                              gl::TEXTURE_WRAP_S,
                              gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE,
                              gl::TEXTURE_WRAP_T,
                              gl::CLAMP_TO_EDGE as GLint);

            gl::BindTexture(gl::TEXTURE_RECTANGLE, 0);

            debug_assert_eq!(gl::GetError(), gl::NO_ERROR);

            texture
        }
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface)
        }

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::DeleteFramebuffers(1, &surface.framebuffer_object);
            surface.renderbuffers.destroy();
            gl::DeleteTextures(1, &surface.texture_object);
        }

        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        unsafe {
            gl::DeleteTextures(1, &surface_texture.texture_object);
            surface_texture.texture_object = 0;
        }

        Ok(surface_texture.surface)
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceId {
        SurfaceId(self.io_surface.as_concrete_TypeRef() as usize)
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
        gl::TEXTURE_RECTANGLE
    }
}

pub(crate) enum Framebuffer {
    // No framebuffer has been attached to the context.
    None,
    // The context is externally-managed.
    External,
    // The context renders to a surface.
    Surface(Surface),
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
    pub(crate) fn new(size: &Size2D<i32>, attributes: &ContextAttributes, info: &GLInfo)
                      -> Renderbuffers {
        unsafe {
            if attributes.flags.contains(ContextAttributeFlags::DEPTH |
                                         ContextAttributeFlags::STENCIL) &&
                    info.features.contains(FeatureFlags::SUPPORTS_DEPTH24_STENCIL8) {
                let mut renderbuffer = 0;
                gl::GenRenderbuffers(1, &mut renderbuffer);
                gl::BindRenderbuffer(gl::RENDERBUFFER, renderbuffer);
                gl::RenderbufferStorage(gl::RENDERBUFFER,
                                        gl::DEPTH24_STENCIL8,
                                        size.width,
                                        size.height);
                gl::BindRenderbuffer(gl::RENDERBUFFER, 0);
                return Renderbuffers::CombinedDepthStencil(renderbuffer);
            }

            let (mut depth_renderbuffer, mut stencil_renderbuffer) = (0, 0);
            if attributes.flags.contains(ContextAttributeFlags::DEPTH) {
                gl::GenRenderbuffers(1, &mut depth_renderbuffer);
                gl::BindRenderbuffer(gl::RENDERBUFFER, depth_renderbuffer);
                gl::RenderbufferStorage(gl::RENDERBUFFER,
                                        gl::DEPTH_COMPONENT24,
                                        size.width,
                                        size.height);
            }
            if attributes.flags.contains(ContextAttributeFlags::STENCIL) {
                gl::GenRenderbuffers(1, &mut stencil_renderbuffer);
                gl::BindRenderbuffer(gl::RENDERBUFFER, stencil_renderbuffer);
                gl::RenderbufferStorage(gl::RENDERBUFFER,
                                        gl::STENCIL_INDEX8,
                                        size.width,
                                        size.height);
            }
            gl::BindRenderbuffer(gl::RENDERBUFFER, 0);

            Renderbuffers::IndividualDepthStencil {
                depth: depth_renderbuffer,
                stencil: stencil_renderbuffer,
            }
        }
    }

    pub(crate) fn bind_to_current_framebuffer(&self) {
        unsafe {
            match *self {
                Renderbuffers::CombinedDepthStencil(renderbuffer) => {
                    if renderbuffer != 0 {
                        gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
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
                        gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                                    gl::DEPTH_ATTACHMENT,
                                                    gl::RENDERBUFFER,
                                                    depth_renderbuffer);
                    }
                    if stencil_renderbuffer != 0 {
                        gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                                    gl::STENCIL_ATTACHMENT,
                                                    gl::RENDERBUFFER,
                                                    stencil_renderbuffer);
                    }
                }
            }
        }
    }

    pub(crate) fn destroy(&mut self) {
        unsafe {
            gl::BindRenderbuffer(gl::RENDERBUFFER, 0);
            match *self {
                Renderbuffers::CombinedDepthStencil(ref mut renderbuffer) => {
                    if *renderbuffer != 0 {
                        gl::DeleteRenderbuffers(1, renderbuffer);
                        *renderbuffer = 0;
                    }
                }
                Renderbuffers::IndividualDepthStencil {
                    depth: ref mut depth_renderbuffer,
                    stencil: ref mut stencil_renderbuffer,
                } => {
                    if *stencil_renderbuffer != 0 {
                        gl::DeleteRenderbuffers(1, stencil_renderbuffer);
                        *stencil_renderbuffer = 0;
                    }
                    if *depth_renderbuffer != 0 {
                        gl::DeleteRenderbuffers(1, depth_renderbuffer);
                        *depth_renderbuffer = 0;
                    }
                }
            }
        }
    }
}
