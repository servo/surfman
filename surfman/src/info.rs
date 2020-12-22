// surfman/surfman/src/info.rs
//
//! OpenGL information.

use crate::gl;
use crate::Gl;

use std::ffi::CStr;
use std::os::raw::c_char;

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

    #[allow(dead_code)]
    pub(crate) fn current(gl: &Gl) -> GLVersion {
        unsafe {
            let version_string = gl.GetString(gl::VERSION) as *const c_char;
            let version_string = CStr::from_ptr(version_string)
                .to_string_lossy()
                .trim_start_matches("OpenGL ES")
                .trim_start()
                .to_owned();
            let mut version_string_iter = version_string.split(|c| c == '.' || c == ' ');
            let major_version: u8 = version_string_iter
                .next()
                .expect("Where's the major GL version?")
                .parse()
                .expect("Couldn't parse the major GL version!");
            let minor_version: u8 = version_string_iter
                .next()
                .expect("Where's the minor GL version?")
                .parse()
                .expect("Couldn't parse the minor GL version!");
            GLVersion {
                major: major_version,
                minor: minor_version,
            }
        }
    }
}
