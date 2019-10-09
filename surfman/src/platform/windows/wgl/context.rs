// surfman/src/platform/windows/wgl/context.rs
//
//! Wrapper for WGL contexts on Windows.

use crate::gl::types::{GLenum, GLint, GLuint};
use std::borrow::Cow;
use std::ffi::CStr;
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::thread;
use winapi::shared::minwindef::{self, BOOL, FALSE, FLOAT, LPARAM, LPVOID, LRESULT, UINT};
use winapi::shared::minwindef::{WORD, WPARAM};
use winapi::shared::ntdef::{HANDLE, LPCSTR};
use winapi::shared::windef::{HBRUSH, HDC, HGLRC, HWND};
use winapi::um::libloaderapi;
use winapi::um::wingdi::{self, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE};
use winapi::um::wingdi::{PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR};
use winapi::um::wingdi::{wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent};
use winapi::um::winuser::{self, COLOR_BACKGROUND, CS_OWNDC, MSG, WM_CREATE, WM_DESTROY};
use winapi::um::winuser::{WNDCLASSA, WS_OVERLAPPEDWINDOW, WS_VISIBLE};

#[allow(non_snake_case)]
#[derive(Default)]
pub(crate) struct WGLExtensionFunctions {
    ChoosePixelFormatARB: Option<extern "C" fn(hDC: HDC,
                                               piAttribIList: *const c_int,
                                               pfAttribFList: *const FLOAT,
                                               nMaxFormats: UINT,
                                               piFormats: *mut c_int,
                                               nNumFormats: *mut UINT)
                                               -> BOOL>,
    CreateContextAttribsARB: Option<extern "C" fn(hDC: HDC,
                                                  shareContext: HGLRC,
                                                  attribList: *const c_int)
                                                  -> HGLRC>,
    GetExtensionsStringARB: Option<extern "C" fn(hdc: HDC) -> *const c_char>,
    dx_interop_functions: Option<WGLDXInteropExtensionFunctions>,
}

#[allow(non_snake_case)]
pub(crate) struct WGLDXInteropExtensionFunctions {
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
        thread::spawn(extension_loader_thread).join().unwrap()
    };
}

fn extension_loader_thread() -> WGLExtensionFunctions {
    unsafe {
        let instance = libloaderapi::GetModuleHandleA(ptr::null_mut());
        let window_class = WNDCLASSA {
            style: CS_OWNDC,
            lpfnWndProc: Some(extension_loader_window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: COLOR_BACKGROUND as HBRUSH,
            lpszMenuName: ptr::null_mut(),
            lpszClassName: &b"SurfmanFalseWindow\0"[0] as *const u8 as LPCSTR,
        };
        let window_class_atom = winuser::RegisterClassA(&window_class);
        assert_ne!(window_class_atom, 0);

        let mut extension_functions = WGLExtensionFunctions::default();

        let window = winuser::CreateWindowExA(
            0,
            window_class_atom as LPCSTR,
            &b"SurfmanFalseWindow\0"[0] as *const u8 as LPCSTR,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            640,
            480,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            &mut extension_functions as *mut WGLExtensionFunctions as LPVOID);

        let mut msg: MSG = mem::zeroed();
        while winuser::GetMessageA(&mut msg, window, 0, 0) != FALSE {
            winuser::TranslateMessage(&msg);
            winuser::DispatchMessageA(&msg);
            if minwindef::LOWORD(msg.message) as UINT == WM_DESTROY {
                break;
            }
        }

        extension_functions
    }
}

#[allow(non_snake_case)]
extern "system" fn extension_loader_window_proc(hwnd: HWND,
                                                uMsg: UINT,
                                                wParam: WPARAM,
                                                lParam: LPARAM)
                                                -> LRESULT {
    unsafe {
        match uMsg {
            WM_CREATE => {
                let pixel_format_descriptor = PIXELFORMATDESCRIPTOR {
                    nSize: mem::size_of::<PIXELFORMATDESCRIPTOR>() as WORD,
                    nVersion: 1,
                    dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                    iPixelType: PFD_TYPE_RGBA,
                    cColorBits: 32,
                    cRedBits: 0,
                    cRedShift: 0,
                    cGreenBits: 0,
                    cGreenShift: 0,
                    cBlueBits: 0,
                    cBlueShift: 0,
                    cAlphaBits: 0,
                    cAlphaShift: 0,
                    cAccumBits: 0,
                    cAccumRedBits: 0,
                    cAccumGreenBits: 0,
                    cAccumBlueBits: 0,
                    cAccumAlphaBits: 0,
                    cDepthBits: 24,
                    cStencilBits: 8,
                    cAuxBuffers: 0,
                    iLayerType: PFD_MAIN_PLANE,
                    bReserved: 0,
                    dwLayerMask: 0,
                    dwVisibleMask: 0,
                    dwDamageMask: 0,
                };

                // Create a false GL context.
                let dc = winuser::GetDC(hwnd);
                let mut pixel_format = wingdi::ChoosePixelFormat(dc, &pixel_format_descriptor);
                assert_ne!(pixel_format, 0);
                let gl_context = wglCreateContext(dc);
                assert!(!gl_context.is_null());
                let ok = wglMakeCurrent(dc, gl_context);
                assert_ne!(ok, FALSE);

                // Detect extensions.
                let wgl_extension_functions = lParam as *mut WGLExtensionFunctions;
                (*wgl_extension_functions).GetExtensionsStringARB = mem::transmute(
                    wglGetProcAddress(&b"wglGetExtensionsStringARB\0"[0] as *const u8 as LPCSTR));
                let extensions = match (*wgl_extension_functions).GetExtensionsStringARB {
                    Some(wglGetExtensionsStringARB) => {
                        CStr::from_ptr(wglGetExtensionsStringARB(dc)).to_string_lossy()
                    }
                    None => Cow::Borrowed(""),
                };

                // Load function pointers.
                for extension in extensions.split(' ') {
                    if extension == "WGL_ARB_pixel_format" {
                        (*wgl_extension_functions).ChoosePixelFormatARB = mem::transmute(
                            wglGetProcAddress(&b"wglChoosePixelFormatARB\0"[0] as *const u8 as
                            LPCSTR));
                        continue;
                    }
                    if extension == "WGL_ARB_create_context" {
                        (*wgl_extension_functions).CreateContextAttribsARB = mem::transmute(
                            wglGetProcAddress(&b"wglCreateContextAttribsARB\0"[0] as *const u8 as
                            LPCSTR));
                        continue;
                    }
                    if extension == "WGL_NV_DX_interop" {
                        (*wgl_extension_functions).dx_interop_functions =
                            Some(WGLDXInteropExtensionFunctions {
                                DXCloseDeviceNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXCloseDeviceNV\0"[0] as *const u8 as LPCSTR)),
                                DXLockObjectsNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXLockObjectsNV\0"[0] as *const u8 as LPCSTR)),
                                DXOpenDeviceNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXOpenDeviceNV\0"[0] as *const u8 as LPCSTR)),
                                DXRegisterObjectNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXRegisterObjectNV\0"[0] as *const u8 as LPCSTR)),
                                DXSetResourceShareHandleNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXSetResourceShareHandleNV\0"[0] as *const u8 as
                                    LPCSTR)),
                                DXUnlockObjectsNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXUnlockObjectsNV\0"[0] as *const u8 as LPCSTR)),
                                DXUnregisterObjectNV: mem::transmute(wglGetProcAddress(
                                    &b"wglDXUnregisterObjectNV\0"[0] as *const u8 as LPCSTR)),
                            });
                        continue;
                    }
                }

                wglDeleteContext(gl_context);
                winuser::DestroyWindow(hwnd);
                0
            }
            _ => winuser::DefWindowProcA(hwnd, uMsg, wParam, lParam),
        }
    }
}
