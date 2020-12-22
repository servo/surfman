// surfman/surfman/src/platform/macos/cgl/error.rs
//
//! Translation of errors from the CGL API to `surfman` errors.

use crate::WindowingApiError;
use cgl::CGLError;

pub(crate) trait ToWindowingApiError {
    fn to_windowing_api_error(self) -> WindowingApiError;
}

impl ToWindowingApiError for CGLError {
    fn to_windowing_api_error(self) -> WindowingApiError {
        match self {
            10000 => WindowingApiError::BadAttribute,
            10001 => WindowingApiError::BadProperty,
            10002 => WindowingApiError::BadPixelFormat,
            10003 => WindowingApiError::BadRendererInfo,
            10004 => WindowingApiError::BadContext,
            10005 => WindowingApiError::BadDrawable,
            10006 => WindowingApiError::BadDisplay,
            10007 => WindowingApiError::BadState,
            10008 => WindowingApiError::BadValue,
            10009 => WindowingApiError::BadMatch,
            10010 => WindowingApiError::BadEnumeration,
            10011 => WindowingApiError::BadOffScreen,
            10012 => WindowingApiError::BadFullScreen,
            10013 => WindowingApiError::BadWindow,
            10014 => WindowingApiError::BadAddress,
            10015 => WindowingApiError::BadCodeModule,
            10016 => WindowingApiError::BadAlloc,
            10017 => WindowingApiError::BadConnection,
            _ => WindowingApiError::Failed,
        }
    }
}
