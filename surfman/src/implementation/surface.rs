// surfman/surfman/src/implementation/surface.rs
//
//! This is an included private module that automatically produces the implementations of the
//! `Surface` and `SurfaceTexture` traits for a backend.

use crate::gl::types::GLuint;
use crate::surface::{Surface as SurfaceInterface, SurfaceTexture as SurfaceTextureInterface};
use crate::{ContextID, SurfaceID};
use super::super::surface::{Surface, SurfaceTexture};

use euclid::default::Size2D;

impl SurfaceInterface for Surface {
    #[inline]
    fn size(&self) -> Size2D<i32> {
        Surface::size(self)
    }

    #[inline]
    fn id(&self) -> SurfaceID {
        Surface::id(self)
    }
    
    #[inline]
    fn context_id(&self) -> ContextID {
        Surface::context_id(self)
    }
    
    #[inline]
    fn framebuffer_object(&self) -> GLuint {
        Surface::framebuffer_object(self)
    }
}

impl SurfaceTextureInterface for SurfaceTexture {
    #[inline]
    fn gl_texture(&self) -> GLuint {
        SurfaceTexture::gl_texture(self)
    }
}
