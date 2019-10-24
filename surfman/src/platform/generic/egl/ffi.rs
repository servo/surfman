// surfman/surfman/src/platform/generic/egl/ffi.rs
//
//! FFI-related functionality common to the various EGL backends.

use crate::egl::{EGLImageKHR, EGLenum};
use super::device;

#[allow(non_snake_case)]
pub(crate) struct EGLExtensionFunctions {
    pub(crate) ImageTargetTexture2DOES: extern "C" fn(target: EGLenum, image: EGLImageKHR),
}

lazy_static! {
    pub(crate) static ref EGL_EXTENSION_FUNCTIONS: EGLExtensionFunctions = {
        let get = device::lookup_egl_extension;
        unsafe {
            EGLExtensionFunctions {
                ImageTargetTexture2DOES: get(b"glEGLImageTargetTexture2DOES\0"),
            }
        }
    };
}
