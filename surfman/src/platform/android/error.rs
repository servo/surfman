//! Translation of errors from the OpenGL API to `surfman` errors.

use crate::gl::types::{GLint, GLuint};
use crate::{WindowingApiError, gl};

impl WindowingApiError {
    pub(crate) fn from_gl_error(gl_error: GLint) -> WindowingApiError {
        match gl_error as GLuint {
            gl::INVALID_ENUM => WindowingApiError::BadEnumeration,
            gl::INVALID_VALUE => WindowingApiError::BadValue,
            gl::INVALID_OPERATION => WindowingApiError::BadOperation,
            _ => WindowingApiError::Failed,
        }
    }
}
