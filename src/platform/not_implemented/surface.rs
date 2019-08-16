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
        let texture = gl.gen_textures(1)[0];
        debug_assert_ne!(texture, 0);

        gl.bind_texture(gl::TEXTURE_2D, texture);
        gl.tex_image_2d(gl::TEXTURE_2D,
                        0,
                        formats.texture_internal as GLint,
                        size.width,
                        size.height,
                        0,
                        formats.texture,
                        formats.texture_type,
                        None);

        // Low filtering to allow rendering
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

        // TODO(emilio): Check if these two are neccessary, probably not
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

        gl.bind_texture(gl::TEXTURE_2D, 0);

        debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

        NativeSurface {
            texture: Some(Arc::new(GLTexture(texture))),
            size: *size,
            formats: *formats,
        }
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn formats(&self) -> &GLFormats {
        &self.formats
    }

    #[inline]
    pub fn destroy(&mut self, gl: &Gl) {
        self.texture = None;
        TEXTURE_TO_DESTROY.with(|texture| {
            gl.delete_textures(&[texture.get()]);
            texture.set(0);
        });
    }
}

impl NativeSurfaceTexture {
    #[inline]
    pub fn new(gl: &dyn Gl, native_surface: NativeSurface) -> NativeSurfaceTexture {
        NativeSurfaceTexture { surface: native_surface, phantom: PhantomData }
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.surface.texture.as_ref().unwrap().0
    }

    #[inline]
    pub fn gl_texture_target() -> GLenum {
        gl::TEXTURE_2D
    }

    #[inline]
    pub fn destroy(&mut self, gl: &Gl) {
        self.surface.destroy(gl);
    }

    #[inline]
    pub fn surface(&self) -> &NativeSurface {
        &self.surface
    }

    #[inline]
    pub fn into_surface(mut self, gl: &dyn Gl) -> NativeSurface {
        self.destroy(gl);
        self.surface
    }
}
