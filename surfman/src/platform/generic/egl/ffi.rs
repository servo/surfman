// surfman/surfman/src/platform/generic/egl/ffi.rs
//
//! FFI-related functionality common to the various EGL backends.

use crate::egl::types::{EGLImageKHR, EGLenum};

#[allow(non_snake_case)]
pub(crate) struct EGLExtensionFunctions {
    pub(crate) ImageTargetTexture2DOES: extern "C" fn(target: EGLenum, image: EGLImageKHR),
}

lazy_static! {
    pub(crate) static ref EGL_EXTENSION_FUNCTIONS: EGLExtensionFunctions = {
        use crate::platform::generic::egl::device::lookup_egl_extension as get;
        use std::mem::transmute as cast;
        unsafe {
            EGLExtensionFunctions {
                ImageTargetTexture2DOES: cast(get(b"glEGLImageTargetTexture2DOES\0")),
            }
        }
    };
}
