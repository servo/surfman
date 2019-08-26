use crate::gl_formats::GLFormats;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLsync, GLuint, Gl};
use std::cell::Cell;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;

thread_local! {
    static TEXTURE_TO_DESTROY: Cell<GLuint> = Cell::new(0);
}

#[derive(Debug)]
struct GLTexture(GLuint);

#[derive(Clone)]
pub struct NativeSurface {
    texture: Option<Arc<GLTexture>>,
    size: Size2D<i32>,
    formats: GLFormats,
}

#[derive(Debug)]
pub struct NativeSurfaceTexture {
    surface: NativeSurface,
    #[allow(dead_code)]
    phantom: PhantomData<*const ()>,
}

unsafe impl Send for NativeSurface {}

impl Drop for GLTexture {
    fn drop(&mut self) {
        TEXTURE_TO_DESTROY.with(|texture| texture.set(self.0));
    }
}

impl Debug for NativeSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?} {:?}, {:?}", self.texture, self.size, self.formats)
    }
}

impl NativeSurface {
    pub fn new(gl: &dyn Gl, size: &Size2D<i32>, formats: &GLFormats) -> NativeSurface {
        panic("NativeSurface::new(): unsupported platform!")
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        unreachable!()
    }

    #[inline]
    pub fn formats(&self) -> &GLFormats {
        unreachable!()
    }

    #[inline]
    pub fn destroy(&mut self, _: &Gl) {
    }
}

impl NativeSurfaceTexture {
    #[inline]
    pub fn new(gl: &dyn Gl, native_surface: NativeSurface) -> NativeSurfaceTexture {
        unreachable!()
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        unreachable!()
    }

    #[inline]
    pub fn gl_texture_target() -> GLenum {
        unreachable!()
    }

    #[inline]
    pub fn destroy(&mut self, _: &Gl) {
    }

    #[inline]
    pub fn surface(&self) -> &NativeSurface {
        unreachable!()
    }

    #[inline]
    pub fn into_surface(mut self, _: &dyn Gl) -> NativeSurface {
        unreachable!()
    }
}
