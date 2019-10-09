//! Wrapper for WGL contexts on Windows.

use gl::types::{GLenum, GLint, GLuint};
use std::ffi::CStr;
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

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
    }
};

    };
}

fn extension_loader_thread() -> WGLExtensionFunctions {
    unsafe {
        let instance = GetModuleHandle(ptr::null_mut());
        let window_class = WNDCLASSA {
            style: CS_OWNDC,
            lpfnWndProc: extension_loader_window_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: COLOR_BACKGROUND,
            lpszMenuName: ptr::null_mut(),
            lpszClassName: &b"SurfmanFalseWindow\0"[0],
        };
        let window_class_atom = RegisterClassA(&window_class);
        assert_ne!(window_class_atom, 0);

        let mut extension_functions = WGLExtensionFunctions::default();

        let window = CreateWindowExA(0,
                                     window_class_atom,
                                     &b"SurfmanFalseWindow\0"[0],
                                     WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                                     0,
                                     0,
                                     640,
                                     480,
                                     0,
                                     ptr::null_mut(),
                                     instance,
                                     &mut extension_functions as LPVOID);

        let mut msg: MSG = mem::zeroed();
        while (GetMessage(&mut msg, window, 0, 0) != FALSE) {
            TranslateMessage(&msg);
            DispatchMessage(&msg);
            if LOWORD(msg.message) == WM_DESTROY {
                break;
            }
        }

        extension_functions
    }
}

extern "C" fn extension_loader_window_proc(hwnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM)
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
                let dc = GetDC(hwnd);
                let mut pixel_format = ChoosePixelFormat(dc, &pixel_format_descriptor);
                assert_ne!(pixel_format, 0);
                let gl_context = wglCreateContext(dc);
                assert!(!gl_context.is_null());
                let ok = wglMakeCurrent(dc, gl_context);
                assert_ne!(ok, FALSE);

                // Detect extensions.
                let wgl_extension_functions = lparam as *mut WGLExtensionFunctions;
                (*wgl_extension_functions).GetExtensionsStringARB =
                    mem::transmute(wglGetProcAddress(&b"wglGetExtensionsStringARB\0"[0]));
                let extensions = match (*wgl_extension_functions).GetExtensionsStringARB {
                    Some(wglGetExtensionsStringARB) => {
                        CStr::from_ptr(wglGetExtensionsStringARB(dc)).to_string_lossy()
                    }
                    None => Cow::Borrowed(""),
                };

                // Load function pointers.
                for extension in extensions.split(' ') {
                    if extension == "WGL_ARB_pixel_format" {
                        (*wgl_extension_functions).ChoosePixelFormatARB =
                            mem::transmute(wglGetProcAddress(&b"wglChoosePixelFormatARB\0"[0]));
                        continue;
                    }
                    if extension == "WGL_ARB_create_context" {
                        (*wgl_extension_functions).CreateContextAttribsARB =
                            mem::transmute(wglGetProcAddress(&b"wglCreateContextAttribsARB\0"[0]));
                        continue;
                    }
                    if extension == "WGL_NV_DX_interop" {
                        (*wgl_extension_functions).dx_interop_functions =
                            Some(WGLDXInteropExtensionFunctions {
                                DXCloseDeviceNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXCloseDeviceNV\0"[0])),
                                DXLockObjectsNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXLockObjectsNV\0"[0])),
                                DXOpenDeviceNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXOpenDeviceNV\0"[0])),
                                DXRegisterObjectNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXRegisterObjectNV\0"[0])),
                                DXSetResourceShareHandleNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXSetResourceShareHandleNV\0"[0])),
                                DXUnlockObjectsNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXUnlockObjectsNV\0"[0])),
                                DXUnregisterObjectNV: mem::transmute(
                                    wglGetProcAddress(&"wglDXUnregisterObjectNV\0"[0])),
                            });
                        continue;
                    }
                }

                wglDeleteContext(gl_context);
                DestroyWindow(hwnd);
                0
            }
            _ => DefWindowProc(hwnd, uMsg, wParam, lParam),
        }
    }
}
