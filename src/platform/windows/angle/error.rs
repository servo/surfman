//! Translation of errors from the CGL API to `surfman` errors.

use crate::WindowingApiError;
use egl::EGLint;

pub(crate) trait ToWindowingApiError {
    fn to_windowing_api_error(self) -> WindowingApiError;
}

impl ToWindowingApiError for EGLint {
    fn to_windowing_api_error(self) -> WindowingApiError {
        // TODO(pcwalton)
        WindowingApiError::Failed
    }
}
