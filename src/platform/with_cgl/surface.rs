//! Surface management for macOS.

use crate::gl_context::GLVersion;
use crate::gl_formats::Format;
use crate::platform::with_cgl::Display;
use crate::surface::SurfaceDescriptor;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl, GlType};
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement};
use io_surface::{kIOSurfaceBytesPerRow, kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::thread;

#[derive(Clone)]
pub struct Surface {
    pub(crate) io_surface: IOSurface,
    pub(crate) descriptor: Arc<SurfaceDescriptor>,
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
