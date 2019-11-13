// surfman/surfman/src/info.rs
//
//! OpenGL information.

/// The API (OpenGL or OpenGL ES).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GLApi {
    /// OpenGL (full or desktop OpenGL).
    GL,
    /// OpenGL ES (embedded OpenGL).
    GLES,
}

/// Describes the OpenGL version that is requested when a context is created.
/// 
/// Since OpenGL and OpenGL ES have different version numbering schemes, the valid values here
/// depend on the value of `Device::gl_api()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GLVersion {
    /// The major OpenGL version (e.g. 4 in 4.2).
    pub major: u8,
    /// The minor OpenGL version (e.g. 2 in 4.2).
    pub minor: u8,
}

impl GLVersion {
    /// Creates a GL version structure with the given major and minor version numbers.
    #[inline]
    pub fn new(major: u8, minor: u8) -> GLVersion {
        GLVersion { major, minor }
    }
}
