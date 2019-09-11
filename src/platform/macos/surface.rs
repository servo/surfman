//! Surface management for macOS.

use crate::{ContextAttributeFlags, ContextAttributes, Error, FeatureFlags, GLInfo, SurfaceId};
use super::context::{Context, ContextDescriptor};
use super::device::Device;

use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use gl;
use gl::types::{GLenum, GLint, GLuint};
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow};
use io_surface::{kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Clone)]
pub struct Surface {
    pub(crate) io_surface: IOSurface,
    pub(crate) descriptor: ContextDescriptor,
    pub(crate) size: Size2D<i32>,
    pub(crate) destroyed: bool,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.io_surface.as_concrete_TypeRef() as usize)
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
    pub fn create_surface(&mut self, descriptor: &ContextDescriptor, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        let io_surface = unsafe {
            let props = CFDictionary::from_CFType_pairs(&[
                (CFString::wrap_under_get_rule(kIOSurfaceWidth),
                 CFNumber::from(size.width).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceHeight),
                 CFNumber::from(size.height).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                 CFNumber::from(BYTES_PER_PIXEL).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                 CFNumber::from(size.width * BYTES_PER_PIXEL).as_CFType()),
            ]);
            io_surface::new(&props)
        };

        Ok(Surface {
            io_surface,
            descriptor: (*descriptor).clone(),
            size: *size,
            destroyed: false,
        })
    }

    pub fn create_surface_texture(&self, _: &mut Context, native_surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        unsafe {
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl::BindTexture(gl::TEXTURE_RECTANGLE, texture);

            let size = native_surface.size();
            let has_alpha = self.context_descriptor_attributes(&native_surface.descriptor())
                                .flags
                                .contains(ContextAttributeFlags::ALPHA);
            native_surface.io_surface.bind_to_gl_texture(size.width, size.height, has_alpha);

            // Low filtering to allow rendering
            gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

            // TODO(emilio): Check if these two are neccessary, probably not
            gl::TexParameteri(gl::TEXTURE_RECTANGLE,
                              gl::TEXTURE_WRAP_S,
                              gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE,
                              gl::TEXTURE_WRAP_T,
                              gl::CLAMP_TO_EDGE as GLint);

            gl::BindTexture(gl::TEXTURE_RECTANGLE, 0);

            debug_assert_eq!(gl::GetError(), gl::NO_ERROR);

            Ok(SurfaceTexture {
                surface: native_surface,
                gl_texture: texture,
                phantom: PhantomData,
            })
        }
    }

    pub fn destroy_surface(&self, mut surface: Surface) -> Result<(), Error> {
        // Nothing to do here.
        surface.destroyed = true;
        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        unsafe {
            gl::DeleteTextures(1, &surface_texture.gl_texture);
            surface_texture.gl_texture = 0;
        }

        Ok(surface_texture.surface)
    }
}

impl Surface {
    #[inline]
    pub fn descriptor(&self) -> ContextDescriptor {
        self.descriptor.clone()
    }

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
        self.gl_texture
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
    // The context renders to an OpenGL framebuffer object backed by an `IOSurface`.
    Object {
        framebuffer_object: GLuint,
        color_surface_texture: SurfaceTexture,
        renderbuffers: Renderbuffers,
    },
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
                        println!("binding depth/stencil renderbuffer: {}", renderbuffer);
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
                        println!("binding depth renderbuffer: {}", depth_renderbuffer);
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
