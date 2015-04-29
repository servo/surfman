use gleam::gl::types::GLenum;
use GLContextCapabilities;

pub struct GLFormats {
    renderbuffer_color: GLenum,
    texture_internal: GLenum,
    texture: GLenum,
    texture_type: GLenum,
}

impl GLFormats {
    fn detect(capabilities: &GLContextCapabilities, is_gles: bool) -> GLFormats {
        unimplemented!()
    }
}

