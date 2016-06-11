use gleam::gl;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct GLLimits {
    pub max_vertex_attribs: u32,
    pub max_tex_size: u32,
    pub max_cube_map_tex_size: u32
}

#[cfg(feature = "serde")]
impl Deserialize for GLLimits {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let max_vertex_attribs = try!(u32::deserialize(deserializer));
        let max_tex_size = try!(u32::deserialize(deserializer));
        let max_cube_map_tex_size = try!(u32::deserialize(deserializer));
        Ok(GLLimits {
            max_vertex_attribs: max_vertex_attribs,
            max_tex_size: max_tex_size,
            max_cube_map_tex_size: max_cube_map_tex_size,
        })
    }
}

#[cfg(feature = "serde")]
impl Serialize for GLLimits {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        try!(self.max_vertex_attribs.serialize(serializer));
        try!(self.max_tex_size.serialize(serializer));
        try!(self.max_cube_map_tex_size.serialize(serializer));
        Ok(())
    }
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
