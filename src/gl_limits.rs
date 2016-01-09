use gleam::gl::types::GLint;
use gleam::gl;

#[derive(Clone, Deserialize, Serialize)]
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
