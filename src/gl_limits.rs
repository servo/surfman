//! Various OpenGL limitations, cached so we don't have to repeatedly query them.

use gl;
use gl::types::GLenum;

#[derive(Debug, Clone, Copy)]
pub struct GLLimits {
    pub max_vertex_attribs: u32,
    pub max_tex_size: u32,
    pub max_cube_map_tex_size: u32,
    pub max_combined_texture_image_units: u32,
    pub max_fragment_uniform_vectors: u32,
    pub max_renderbuffer_size: u32,
    pub max_texture_image_units: u32,
    pub max_varying_vectors: u32,
    pub max_vertex_texture_image_units: u32,
    pub max_vertex_uniform_vectors: u32,
    pub max_samples: u32,
}

fn gl_integer(pname: GLenum) -> u32 {
    gl_fallible_integer(pname).unwrap()
}

fn gl_fallible_integer(pname: GLenum) -> Result<u32, ()> {
    let mut value = 0;
    unsafe {
        gl::GetIntegerv(pname, &mut value);

        match gl::GetError() {
            gl::NO_ERROR => Ok(value as u32),
            gl::INVALID_ENUM => Err(()),
            _ => panic!("Got unexpected error from glGetIntegerv()!"),
        }
    }
}

impl GLLimits {
    pub fn detect() -> GLLimits {
        let max_vertex_attribs = gl_integer(gl::MAX_VERTEX_ATTRIBS);
        let max_tex_size = gl_integer(gl::MAX_TEXTURE_SIZE);
        let max_cube_map_tex_size = gl_integer(gl::MAX_CUBE_MAP_TEXTURE_SIZE);
        let max_combined_texture_image_units = gl_integer(gl::MAX_COMBINED_TEXTURE_IMAGE_UNITS);
        let max_renderbuffer_size = gl_integer(gl::MAX_RENDERBUFFER_SIZE);
        let max_texture_image_units = gl_integer(gl::MAX_TEXTURE_IMAGE_UNITS);
        let max_vertex_texture_image_units = gl_integer(gl::MAX_VERTEX_TEXTURE_IMAGE_UNITS);
        let max_samples = gl_integer(gl::MAX_SAMPLES);

        // Based on:
        // https://searchfox.org/mozilla-central/rev/5a744713370ec47969595e369fd5125f123e6d24/dom/canvas/WebGLContextValidate.cpp#523-558
        let (max_fragment_uniform_vectors, max_varying_vectors, max_vertex_uniform_vectors) =
            match gl_fallible_integer(gl::MAX_FRAGMENT_UNIFORM_VECTORS) {
                Ok(limit) => {
                    (limit,
                     gl_integer(gl::MAX_VARYING_VECTORS),
                     gl_integer(gl::MAX_VERTEX_UNIFORM_VECTORS))
                }
                Err(()) => {
                    let max_fragment_uniform_components =
                        gl_integer(gl::MAX_FRAGMENT_UNIFORM_COMPONENTS);
                    let max_vertex_uniform_components =
                        gl_integer(gl::MAX_VERTEX_UNIFORM_COMPONENTS);

                    let max_vertex_output_components =
                        gl_fallible_integer(gl::MAX_VERTEX_OUTPUT_COMPONENTS).unwrap_or(0);
                    let max_fragment_input_components =
                        gl_fallible_integer(gl::MAX_FRAGMENT_INPUT_COMPONENTS).unwrap_or(0);
                    let max_varying_components =
                        max_vertex_output_components.min(max_fragment_input_components).max(16);

                        (max_fragment_uniform_components / 4,
                        max_varying_components / 4,
                        max_vertex_uniform_components / 4)
                }
            };

        GLLimits {
            max_vertex_attribs,
            max_tex_size,
            max_cube_map_tex_size,
            max_combined_texture_image_units,
            max_fragment_uniform_vectors,
            max_renderbuffer_size,
            max_texture_image_units,
            max_varying_vectors,
            max_vertex_texture_image_units,
            max_vertex_uniform_vectors,
            max_samples,
        }
    }
}
