//! Wrapper for WGL contexts on Windows.

use gl::types::{GLenum, GLint, GLuint};
use std::os::raw::{c_int, c_void};

pub(crate) struct WGLExtensionFunctions {
    ChoosePixelFormatARB: extern "C" fn(hDC: HDC,
                                        piAttribIList: *const c_int,
                                        pfAttribFList: *const FLOAT,
                                        nMaxFormats: UINT,
                                        piFormats: *mut c_int,
                                        nNumFormats: *mut UINT)
                                        -> BOOL,
    CreateContextAttribsARB: extern "C" fn(hDC: HDC,
                                           shareContext: HGLRC,
                                           attrib_list: *const c_int)
                                           -> HGLRC,
    DXCloseDeviceNV: extern "C" fn(hDevice: HANDLE) -> BOOL,
    DXLockObjectsNV: extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE) -> BOOL,
    DXOpenDeviceNV: extern "C" fn(dxDevice: *mut c_void) -> HANDLE,
    DXRegisterObjectNV: extern "C" fn(hDevice: HANDLE,
                                      dxResource: *mut c_void,
                                      name: GLuint,
                                      object_type: GLenum,
                                      access: GLenum)
                                      -> HANDLE,
    DXSetResourceShareHandleNV: extern "C" fn(dxResource: *mut c_void, shareHandle: HANDLE)
                                              -> BOOL,
    DXUnlockObjectsNV: extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE) -> BOOL,
    DXUnregisterObjectNV: extern "C" fn(hObject: HANDLE) -> BOOL,
}

lazy_static! {
    pub(crate) static ref WGL_EXTENSION_FUNCTIONS: WGLExtensionFunctions = {
    };
}
