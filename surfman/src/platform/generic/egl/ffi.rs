// surfman/surfman/src/platform/generic/egl/ffi.rs
//
//! FFI-related functionality common to the various EGL backends.

use crate::egl::types::{EGLAttrib, EGLBoolean, EGLContext, EGLDisplay, EGLenum, EGLint};

use std::os::raw::c_void;

enum EGLClientBufferOpaque {}
pub type EGLClientBufferObject = *mut EGLClientBufferOpaque;

enum EGLDeviceEXTOpaque {}
pub type EGLDeviceEXT = *mut EGLDeviceEXTOpaque;

enum EGLImageKHROpaque {}
pub type EGLImageKHR = *mut EGLImageKHROpaque;

pub const EGL_PLATFORM_DEVICE_EXT:               EGLenum = 0x313f;
pub const EGL_NATIVE_BUFFER_ANDROID:             EGLenum = 0x3140;
pub const EGL_DRM_BUFFER_FORMAT_MESA:            EGLenum = 0x31d0;
pub const EGL_DRM_BUFFER_USE_MESA:               EGLenum = 0x31d1;
pub const EGL_DRM_BUFFER_FORMAT_ARGB32_MESA:     EGLenum = 0x31d2;
pub const EGL_DRM_BUFFER_MESA:                   EGLenum = 0x31d3;
pub const EGL_DRM_BUFFER_STRIDE_MESA:            EGLenum = 0x31d4;
pub const EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE: EGLenum = 0x3200;
pub const EGL_D3D11_DEVICE_ANGLE:                EGLenum = 0x33a1;
pub const EGL_DXGI_KEYED_MUTEX_ANGLE:            EGLenum = 0x33a2;

pub const EGL_NO_DEVICE_EXT: EGLDeviceEXT = 0 as EGLDeviceEXT;
pub const EGL_NO_IMAGE_KHR: EGLImageKHR = 0 as EGLImageKHR;

#[allow(non_snake_case)]
pub(crate) struct EGLExtensionFunctions {
    // Ubiquitous extensions assumed to be present
    pub(crate) CreateImageKHR: extern "C" fn(dpy: EGLDisplay,
                                             ctx: EGLContext,
                                             target: EGLenum,
                                             buffer: EGLClientBuffer,
                                             attrib_list: *const EGLint)
                                             -> EGLImageKHR,
    pub(crate) DestroyImageKHR: extern "C" fn(dpy: EGLDisplay, image: EGLImageKHR) -> EGLBoolean,
    pub(crate) ImageTargetTexture2DOES: extern "C" fn(target: EGLenum, image: EGLImageKHR),

    // Optional extensions
    pub(crate) CreateDRMImageMESA: Option<extern "C" fn(dpy: EGLDisplay,
                                                        attrib_list: *const EGLint)
                                                        -> EGLImageKHR>,
    pub(crate) CreateDeviceANGLE: Option<extern "C" fn(device_type: EGLint,
                                                       native_device: *mut c_void,
                                                       attrib_list: *const EGLAttrib)
                                                       -> EGLDeviceEXT>,
    pub(crate) ExportDRMImageMESA: Option<extern "C" fn(dpy: EGLDisplay,
                                                        image: EGLImageKHR,
                                                        name: *mut EGLint,
                                                        handle: *mut EGLint,
                                                        stride: *mut EGLint)
                                                        -> EGLBoolean>,
    pub(crate) GetNativeClientBufferANDROID: Option<extern "C" fn(buffer: *const AHardwareBuffer)
                                                                  -> EGLClientBuffer>,
    pub(crate) QueryDeviceAttribEXT: Option<extern "C" fn(device: EGLDeviceEXT,
                                                          attribute: EGLint,
                                                          value: *mut EGLAttrib)
                                                          -> EGLBoolean>,
    pub(crate) QueryDisplayAttribEXT: Option<extern "C" fn(dpy: EGLDisplay,
                                                           attribute: EGLint,
                                                           value: *mut EGLAttrib)
                                                           -> EGLBoolean>,
    pub(crate) QuerySurfacePointerANGLE: Option<extern "C" fn(dpy: EGLDisplay,
                                                              surface: EGLSurface,
                                                              attribute: EGLint,
                                                              value: *mut *mut c_void)
                                                              -> EGLBoolean>,
}

lazy_static! {
    pub(crate) static ref EGL_EXTENSION_FUNCTIONS: EGLExtensionFunctions = {
        use crate::platform::generic::egl::device::lookup_egl_extension as get;
        use std::mem::transmute as cast;
        unsafe {
            EGLExtensionFunctions {
                CreateImageKHR: cast(get(b"eglCreateImageKHR\0"))),
                DestroyImageKHR: cast(get(b"eglDestroyImageKHR\0")),
                ImageTargetTexture2DOES: cast(get(b"glEGLImageTargetTexture2DOES\0")),

                CreateDRMImageMESA: cast(get(b"eglCreateDRMImageMESA\0")),
                CreateDeviceANGLE: cast(get(b"eglCreateDeviceANGLE\0")),
                ExportDRMImageMESA: cast(get(b"eglExportDRMImageMESA\0")),
                GetNativeClientBufferANDROID: cast(get(b"eglGetNativeClientBufferANDROID\0")),
                QueryDeviceAttribEXT: cast(get(b"eglQueryDeviceAttribEXT\0")),
                QueryDisplayAttribEXT: cast(get(b"eglQueryDisplayAttribEXT\0")),
                QuerySurfacePointerANGLE: cast(get(b"eglQuerySurfacePointerANGLE\0")),
            }
        }
    };
}
