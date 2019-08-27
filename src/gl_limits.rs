use gleam::gl;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for GLLimits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let values = (<[_; 10]>::deserialize(deserializer))?;
        Ok(GLLimits {
            max_vertex_attribs: values[0],
            max_tex_size: values[1],
            max_cube_map_tex_size: values[2],
            max_combined_texture_image_units: values[3],
            max_fragment_uniform_vectors: values[4],
            max_renderbuffer_size: values[5],
            max_texture_image_units: values[6],
            max_varying_vectors: values[7],
            max_vertex_texture_image_units: values[8],
            max_vertex_uniform_vectors: values[9],
        })
    }
}

#[cfg(feature = "serde")]
impl Serialize for GLLimits {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        [self.max_vertex_attribs, self.max_tex_size, self.max_cube_map_tex_size,
         self.max_combined_texture_image_units, self.max_fragment_uniform_vectors,
         self.max_renderbuffer_size, self.max_texture_image_units, self.max_varying_vectors,
         self.max_vertex_texture_image_units, self.max_vertex_uniform_vectors]
            .serialize(serializer)
    }
}

macro_rules! gl_integer {
    ($gl:ident, $pname:ident) => {
        {
            debug_assert!($gl.get_error() == gl::NO_ERROR);
            let mut val = [0];
            unsafe {
                $gl.get_integer_v(gl::$pname, &mut val);
            }
            assert_eq!($gl.get_error(), gl::NO_ERROR, "Error retrieving {}", stringify!($pname));
            val[0] as u32
        }
    }
}

fn gl_fallible_integer(gl_: &dyn gl::Gl, pname: gl::GLenum) -> Result<u32, ()> {
    let mut val = [0];
    unsafe {
        gl_.get_integer_v(pname, &mut val);
    }
    let err = gl_.get_error();
    if err == gl::INVALID_ENUM {
        return Err(());
    }
    assert_eq!(err, gl::NO_ERROR);
    Ok(val[0] as u32)
}

impl GLLimits {
    pub fn detect(gl_: &dyn gl::Gl) -> GLLimits {
        let max_vertex_attribs = gl_integer!(gl_, MAX_VERTEX_ATTRIBS);
        let max_tex_size = gl_integer!(gl_, MAX_TEXTURE_SIZE);
        let max_cube_map_tex_size = gl_integer!(gl_, MAX_CUBE_MAP_TEXTURE_SIZE);
        let max_combined_texture_image_units = gl_integer!(gl_, MAX_COMBINED_TEXTURE_IMAGE_UNITS);
        let max_renderbuffer_size = gl_integer!(gl_, MAX_RENDERBUFFER_SIZE);
        let max_texture_image_units = gl_integer!(gl_, MAX_TEXTURE_IMAGE_UNITS);
        let max_vertex_texture_image_units = gl_integer!(gl_, MAX_VERTEX_TEXTURE_IMAGE_UNITS);

        // Based off of https://searchfox.org/mozilla-central/rev/5a744713370ec47969595e369fd5125f123e6d24/dom/canvas/WebGLContextValidate.cpp#523-558
        let (max_fragment_uniform_vectors,
             max_varying_vectors,
             max_vertex_uniform_vectors) = match gl_fallible_integer(gl_, gl::MAX_FRAGMENT_UNIFORM_VECTORS) {
            Ok(limit) => {
                (limit,
                 gl_integer!(gl_, MAX_VARYING_VECTORS),
                 gl_integer!(gl_, MAX_VERTEX_UNIFORM_VECTORS))
            }
            Err(()) => {
                let max_fragment_uniform_components =
                    gl_integer!(gl_, MAX_FRAGMENT_UNIFORM_COMPONENTS);
                let max_vertex_uniform_components =
                    gl_integer!(gl_, MAX_VERTEX_UNIFORM_COMPONENTS);

                let max_vertex_output_components =
                    gl_fallible_integer(gl_, gl::MAX_VERTEX_OUTPUT_COMPONENTS).unwrap_or(0);
                let max_fragment_input_components =
                    gl_fallible_integer(gl_, gl::MAX_FRAGMENT_INPUT_COMPONENTS).unwrap_or(0);
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
        }
    }
}
