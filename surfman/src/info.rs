//! OpenGL information.

/// The API (OpenGL or OpenGL ES).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GLApi {
    GL,
    GLES,
}

/// Describes the OpenGL version that is requested when a context is created.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GLVersion {
    pub major: u8,
    pub minor: u8,
}

impl GLVersion {
    #[inline]
    pub fn new(major: u8, minor: u8) -> GLVersion {
        GLVersion { major, minor }
    }
}
