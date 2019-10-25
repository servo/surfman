// surfman/surfman/src/platform/unix/wayland/ffi.rs
//
//! FFI definitions specific to the Wayland backend.

use crate::egl::types::{EGLBoolean, EGLDisplay, EGLImageKHR, EGLenum, EGLint};
use crate::platform::generic::egl::device;

// EGL_MESA_drm_image

pub const EGL_DRM_BUFFER_FORMAT_MESA:        EGLenum = 0x31d0;
pub const EGL_DRM_BUFFER_USE_MESA:           EGLenum = 0x31d1;
pub const EGL_DRM_BUFFER_FORMAT_ARGB32_MESA: EGLenum = 0x31d2;
pub const EGL_DRM_BUFFER_MESA:               EGLenum = 0x31d3;
pub const EGL_DRM_BUFFER_STRIDE_MESA:        EGLenum = 0x31d4;

#[allow(non_snake_case)]
pub(crate) struct EGLExtensionFunctions {
    pub(crate) CreateDRMImageMESA: extern "C" fn(dpy: EGLDisplay, attrib_list: *const EGLint)
                                                 -> EGLImageKHR,
    pub(crate) ExportDRMImageMESA: extern "C" fn(dpy: EGLDisplay,
                                                 image: EGLImageKHR,
                                                 name: *mut EGLint,
                                                 handle: *mut EGLint,
                                                 stride: *mut EGLint)
                                                 -> EGLBoolean,
}

lazy_static! {
    pub(crate) static ref EGL_EXTENSION_FUNCTIONS: EGLExtensionFunctions = {
        use crate::platform::generic::egl::device::lookup_egl_extension as get;
        use std::mem::transmute as cast;
        unsafe {
            EGLExtensionFunctions {
                CreateDRMImageMESA: cast(get(b"eglCreateDRMImageMESA\0")),
                ExportDRMImageMESA: cast(get(b"eglExportDRMImageMESA\0")),
            }
        }
    };
}
