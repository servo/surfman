// surfman/surfman/src/platform/generic/egl/error.rs

//! Translation of errors from the EGL API to `surfman` errors.

use crate::egl;
use crate::egl::types::{EGLenum, EGLint};
use crate::WindowingApiError;

pub(crate) trait ToWindowingApiError {
    fn to_windowing_api_error(self) -> WindowingApiError;
}

impl ToWindowingApiError for EGLint {
    fn to_windowing_api_error(self) -> WindowingApiError {
        match self as EGLenum {
            egl::NOT_INITIALIZED => WindowingApiError::NotInitialized,
            egl::BAD_ACCESS => WindowingApiError::BadAccess,
            egl::BAD_ALLOC => WindowingApiError::BadAlloc,
            egl::BAD_ATTRIBUTE => WindowingApiError::BadAttribute,
            egl::BAD_CONFIG => WindowingApiError::BadConfig,
            egl::BAD_CONTEXT => WindowingApiError::BadContext,
            egl::BAD_CURRENT_SURFACE => WindowingApiError::BadCurrentSurface,
            egl::BAD_DISPLAY => WindowingApiError::BadDisplay,
            egl::BAD_SURFACE => WindowingApiError::BadSurface,
            egl::BAD_MATCH => WindowingApiError::BadMatch,
            egl::BAD_PARAMETER => WindowingApiError::BadParameter,
            egl::BAD_NATIVE_PIXMAP => WindowingApiError::BadNativePixmap,
            egl::BAD_NATIVE_WINDOW => WindowingApiError::BadNativeWindow,
            egl::CONTEXT_LOST => WindowingApiError::ContextLost,
            _ => WindowingApiError::Failed,
        }
    }
}
