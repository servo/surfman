//! Translation of X11 errors to `surfman` errors.

use crate::WindowingApiError;

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use x11::glx::{GLX_BAD_ATTRIBUTE, GLX_BAD_CONTEXT, GLX_BAD_ENUM, GLX_BAD_SCREEN, GLX_BAD_VALUE};
use x11::glx::{GLX_BAD_VISUAL, GLX_NO_EXTENSION};
use x11::xlib::{Display, XGetErrorText};

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

pub(crate) fn xlib_error_to_windowing_api_error(display: *mut Display, xlib_error: u8)
                                                -> WindowingApiError {
    unsafe {
        let mut error_text: Vec<u8> = vec![0; 256];
        XGetErrorText(display,
                      xlib_error as c_int,
                      error_text.as_mut_ptr() as *mut c_char,
                      error_text.len() as c_int - 1);
        if error_text.starts_with(b"GLXBadFBConfig\0") {
            WindowingApiError::BadPixelFormat
        } else {
            WindowingApiError::Failed
        }
    }
}

