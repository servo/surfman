use gleam::gl::types::GLint;
use gleam::gl;

#[derive(Clone)]
#[cfg_attr(feature="serde_serialization", derive(Serialize, Deserialize))]
pub struct GLLimits {
    pub max_vertex_attribs: GLint,
}

impl GLLimits {
    pub fn detect() -> GLLimits {
        GLLimits {
            max_vertex_attribs: gl::get_integer_v(gl::MAX_VERTEX_ATTRIBS),
        }
    }
}
