use gleam::gl;

#[derive(Debug, Clone)]
#[cfg_attr(feature="serde_serialization", derive(Serialize, Deserialize))]
pub struct GLLimits {
    pub max_vertex_attribs: u32,
    pub max_tex_size: u32,
    pub max_cube_map_tex_size: u32
}

impl GLLimits {
    pub fn detect() -> GLLimits {
        GLLimits {
            max_vertex_attribs: gl::get_integer_v(gl::MAX_VERTEX_ATTRIBS) as u32,
            max_tex_size: gl::get_integer_v(gl::MAX_TEXTURE_SIZE) as u32,
            max_cube_map_tex_size: gl::get_integer_v(gl::MAX_CUBE_MAP_TEXTURE_SIZE) as u32
        }
    }
}
