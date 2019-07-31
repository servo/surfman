use crate::gl_formats::GLFormats;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLsync, GLuint, Gl};
use std::fmt::{self, Debug, Formatter};

#[derive(Clone)]
pub struct NativeSurface {
    texture: GLuint,
    //sync: GLsync,
    size: Size2D<i32>,
    alpha: bool,
}

impl Debug for NativeSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?} {:?}, {:?}", self.texture, self.size, self.alpha)
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
            texture,
            size: *size,
            alpha: formats.has_alpha(),
        }
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn alpha(&self) -> bool {
        self.alpha
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.texture
    }

    #[inline]
    pub fn gl_texture_type(&self) -> GLenum {
        gl::TEXTURE_2D
    }
}
