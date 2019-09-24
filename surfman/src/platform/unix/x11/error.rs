//! Translation of X11 errors to `surfman` errors.

use crate::WindowingApiError;

use std::os::raw::c_int;
use x11::glx::{GLX_BAD_ATTRIBUTE, GLX_BAD_CONTEXT, GLX_BAD_ENUM, GLX_BAD_SCREEN, GLX_BAD_VALUE};
use x11::glx::{GLX_BAD_VISUAL, GLX_NO_EXTENSION};

pub(crate) fn glx_error_to_windowing_api_error(glx_error: c_int) -> WindowingApiError {
    match glx_error {
        GLX_BAD_SCREEN => WindowingApiError::BadScreen,
        GLX_BAD_ATTRIBUTE => WindowingApiError::BadAttribute,
        GLX_NO_EXTENSION => WindowingApiError::NoExtension,
        GLX_BAD_VISUAL => WindowingApiError::BadVisual,
        GLX_BAD_CONTEXT => WindowingApiError::BadContext,
        GLX_BAD_VALUE => WindowingApiError::BadValue,
        GLX_BAD_ENUM => WindowingApiError::BadEnumeration,
        _ => WindowingApiError::Failed,
    }
}
