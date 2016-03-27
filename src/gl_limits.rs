use gleam::gl;

#[derive(Debug, Clone)]
#[cfg_attr(feature="serde_serialization", derive(Serialize, Deserialize))]
pub struct GLLimits {
    pub max_vertex_attribs: u32,
}

impl GLLimits {
    pub fn detect() -> GLLimits {
        GLLimits {
            max_vertex_attribs: gl::get_integer_v(gl::MAX_VERTEX_ATTRIBS) as u32,
        }
    }
}
