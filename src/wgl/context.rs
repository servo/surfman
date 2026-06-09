// surfman/src/platform/windows/wgl/context.rs
//
//! Wrapper for WGL contexts on Windows.

use super::device::HiddenWindow;
use super::surface::Surface;
use crate::gl;
use crate::surface::Framebuffer;
use crate::Gl;
use crate::{ContextID, Error, GLVersion};
use glow::HasContext;
use std::borrow::Cow;
use std::ffi::CStr;
use std::mem;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;
use std::sync::LazyLock;
use std::thread;
use winapi::shared::minwindef::{BOOL, FALSE, FLOAT, HMODULE, LPARAM, LPVOID, LRESULT, UINT};
use winapi::shared::minwindef::{WORD, WPARAM};
use winapi::shared::ntdef::{HANDLE, LPCSTR};
use winapi::shared::windef::{HBRUSH, HDC, HGLRC, HWND};
use winapi::um::libloaderapi;
use winapi::um::wingdi::{self, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE};
use winapi::um::wingdi::{wglCreateContext, wglDeleteContext, wglGetCurrentContext};
use winapi::um::wingdi::{wglGetCurrentDC, wglGetProcAddress, wglMakeCurrent};
use winapi::um::wingdi::{PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR};
use winapi::um::winuser::{self, COLOR_BACKGROUND, CREATESTRUCTA, CS_OWNDC, WM_CREATE, WNDCLASSA};
use winapi::um::winuser::{WS_OVERLAPPEDWINDOW, WS_VISIBLE};

type GLenum = c_uint;
type GLint = c_int;
type GLuint = c_uint;

#[allow(non_snake_case)]
#[derive(Default)]
pub(crate) struct WGLExtensionFunctions {
    pub(crate) CreateContextAttribsARB: Option<
        unsafe extern "C" fn(hDC: HDC, shareContext: HGLRC, attribList: *const c_int) -> HGLRC,
    >,
    pub(crate) GetExtensionsStringARB: Option<unsafe extern "C" fn(hdc: HDC) -> *const c_char>,
    pub(crate) pixel_format_functions: Option<WGLPixelFormatExtensionFunctions>,
    pub(crate) dx_interop_functions: Option<WGLDXInteropExtensionFunctions>,
}

#[allow(non_snake_case)]
pub(crate) struct WGLPixelFormatExtensionFunctions {
    pub(crate) ChoosePixelFormatARB: unsafe extern "C" fn(
        hdc: HDC,
        piAttribIList: *const c_int,
        pfAttribFList: *const FLOAT,
        nMaxFormats: UINT,
        piFormats: *mut c_int,
        nNumFormats: *mut UINT,
    ) -> BOOL,
    pub(crate) GetPixelFormatAttribivARB: unsafe extern "C" fn(
        hdc: HDC,
        iPixelFormat: c_int,
        iLayerPlane: c_int,
        nAttributes: UINT,
        piAttributes: *const c_int,
        piValues: *mut c_int,
    ) -> BOOL,
}

#[allow(non_snake_case)]
pub(crate) struct WGLDXInteropExtensionFunctions {
    pub(crate) DXCloseDeviceNV: unsafe extern "C" fn(hDevice: HANDLE) -> BOOL,
    pub(crate) DXLockObjectsNV:
        unsafe extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE) -> BOOL,
    pub(crate) DXOpenDeviceNV: unsafe extern "C" fn(dxDevice: *mut c_void) -> HANDLE,
    pub(crate) DXRegisterObjectNV: unsafe extern "C" fn(
        hDevice: HANDLE,
        dxResource: *mut c_void,
        name: GLuint,
        object_type: GLenum,
        access: GLenum,
    ) -> HANDLE,
    pub(crate) DXSetResourceShareHandleNV:
        unsafe extern "C" fn(dxResource: *mut c_void, shareHandle: HANDLE) -> BOOL,
    pub(crate) DXUnlockObjectsNV:
        unsafe extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE) -> BOOL,
    pub(crate) DXUnregisterObjectNV: unsafe extern "C" fn(hDevice: HANDLE, hObject: HANDLE) -> BOOL,
}

/// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
///
/// These are local to a device.
#[derive(Clone)]
pub struct ContextDescriptor {
    pub(crate) pixel_format: c_int,
    pub(crate) gl_version: GLVersion,
    pub(crate) compatibility_profile: bool,
}

/// Represents an OpenGL rendering context.
///
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
///
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
///
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
///
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub struct Context {
    pub(crate) glrc: HGLRC,
    pub(crate) id: ContextID,
    pub(crate) gl: Gl,
    pub(crate) hidden_window: Option<HiddenWindow>,
    pub(crate) framebuffer: Framebuffer<Surface, ()>,
    pub(crate) status: ContextStatus,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ContextStatus {
    Owned,
    Referenced,
    Destroyed,
}

/// Wrapper for a WGL `HGLRC`.
#[derive(Clone)]
pub struct NativeContext(pub HGLRC);

thread_local! {
    pub(crate) static OPENGL_LIBRARY: HMODULE = {
        unsafe {
            libloaderapi::LoadLibraryA(c"opengl32.dll".as_ptr())
        }
    };
}

pub(crate) static WGL_EXTENSION_FUNCTIONS: LazyLock<WGLExtensionFunctions> =
    LazyLock::new(|| thread::spawn(extension_loader_thread).join().unwrap());

impl NativeContext {
    /// Returns the current context, if there is one.
    ///
    /// If there is not a native context, this returns a `NoCurrentContext` error.
    #[inline]
    pub fn current() -> Result<NativeContext, Error> {
        unsafe {
            let glrc = wglGetCurrentContext();
            if glrc != ptr::null_mut() {
                Ok(NativeContext(glrc))
            } else {
                Err(Error::NoCurrentContext)
            }
        }
    }
}

fn extension_loader_thread() -> WGLExtensionFunctions {
    unsafe {
        let instance = libloaderapi::GetModuleHandleA(ptr::null_mut());
        let window_class_name = c"SurfmanFalseWindow".as_ptr();
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
            lpszClassName: window_class_name,
        };
        let window_class_atom = winuser::RegisterClassA(&window_class);
        assert_ne!(window_class_atom, 0);

        let mut extension_functions = WGLExtensionFunctions::default();
        // The `lpClassName` parameter of `CreateWindowExA()` takes either
        // a pointer to a null-terminated c string, or an `ATOM` / `u16` encoded
        // in the lower bytes of the pointer type. We do the latter by forcing an
        // `as` cast of the ATOM to the pointer type `LPCSTR`.
        let lp_class_name = window_class_atom as LPCSTR;
        let window = winuser::CreateWindowExA(
            0,
            lp_class_name,
            window_class_name,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            640,
            480,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            &mut extension_functions as *mut WGLExtensionFunctions as LPVOID,
        );

        winuser::DestroyWindow(window);

        extension_functions
    }
}

#[allow(non_snake_case)]
extern "system" fn extension_loader_window_proc(
    hwnd: HWND,
    uMsg: UINT,
    wParam: WPARAM,
    lParam: LPARAM,
) -> LRESULT {
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
                let pixel_format = wingdi::ChoosePixelFormat(dc, &pixel_format_descriptor);
                assert_ne!(pixel_format, 0);
                let mut ok = wingdi::SetPixelFormat(dc, pixel_format, &pixel_format_descriptor);
                assert_ne!(ok, FALSE);
                let gl_context = wglCreateContext(dc);
                assert!(!gl_context.is_null());
                ok = wglMakeCurrent(dc, gl_context);
                assert_ne!(ok, FALSE);

                // Detect extensions.
                let create_struct = lParam as *mut CREATESTRUCTA;
                let wgl_extension_functions =
                    (*create_struct).lpCreateParams as *mut WGLExtensionFunctions;
                (*wgl_extension_functions).GetExtensionsStringARB =
                    mem::transmute(wglGetProcAddress(c"wglGetExtensionsStringARB".as_ptr()));
                let extensions = match (*wgl_extension_functions).GetExtensionsStringARB {
                    Some(wglGetExtensionsStringARB) => {
                        CStr::from_ptr(wglGetExtensionsStringARB(dc)).to_string_lossy()
                    }
                    None => Cow::Borrowed(""),
                };

                // Load function pointers.
                for extension in extensions.split(' ') {
                    if extension == "WGL_ARB_pixel_format" {
                        (*wgl_extension_functions).pixel_format_functions =
                            Some(WGLPixelFormatExtensionFunctions {
                                ChoosePixelFormatARB: mem::transmute(wglGetProcAddress(
                                    c"wglChoosePixelFormatARB".as_ptr(),
                                )),
                                GetPixelFormatAttribivARB: mem::transmute(wglGetProcAddress(
                                    c"wglGetPixelFormatAttribivARB".as_ptr(),
                                )),
                            });
                        continue;
                    }
                    if extension == "WGL_ARB_create_context" {
                        (*wgl_extension_functions).CreateContextAttribsARB = mem::transmute(
                            wglGetProcAddress(c"wglCreateContextAttribsARB".as_ptr()),
                        );
                        continue;
                    }
                    if extension == "WGL_NV_DX_interop" {
                        (*wgl_extension_functions).dx_interop_functions =
                            Some(WGLDXInteropExtensionFunctions {
                                DXCloseDeviceNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXCloseDeviceNV".as_ptr(),
                                )),
                                DXLockObjectsNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXLockObjectsNV".as_ptr(),
                                )),
                                DXOpenDeviceNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXOpenDeviceNV".as_ptr(),
                                )),
                                DXRegisterObjectNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXRegisterObjectNV".as_ptr(),
                                )),
                                DXSetResourceShareHandleNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXSetResourceShareHandleNV".as_ptr(),
                                )),
                                DXUnlockObjectsNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXUnlockObjectsNV".as_ptr(),
                                )),
                                DXUnregisterObjectNV: mem::transmute(wglGetProcAddress(
                                    c"wglDXUnregisterObjectNV".as_ptr(),
                                )),
                            });
                        continue;
                    }
                }

                wglDeleteContext(gl_context);
                0
            }
            _ => winuser::DefWindowProcA(hwnd, uMsg, wParam, lParam),
        }
    }
}

#[must_use]
pub(crate) struct FramebufferGuard<'a> {
    context: &'a Context,
    old_read_framebuffer: Option<glow::Framebuffer>,
    old_draw_framebuffer: Option<glow::Framebuffer>,
}

impl<'a> Drop for FramebufferGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.context
                .gl
                .bind_framebuffer(gl::READ_FRAMEBUFFER, self.old_read_framebuffer);
            self.context
                .gl
                .bind_framebuffer(gl::DRAW_FRAMEBUFFER, self.old_draw_framebuffer);
        }
    }
}

impl<'a> FramebufferGuard<'a> {
    pub(crate) fn new(context: &'a Context) -> FramebufferGuard<'a> {
        unsafe {
            let current_draw_framebuffer = context
                .gl
                .get_parameter_framebuffer(gl::DRAW_FRAMEBUFFER_BINDING);
            let current_read_framebuffer = context
                .gl
                .get_parameter_framebuffer(gl::READ_FRAMEBUFFER_BINDING);

            FramebufferGuard {
                context,
                old_draw_framebuffer: current_draw_framebuffer,
                old_read_framebuffer: current_read_framebuffer,
            }
        }
    }
}

#[must_use]
pub(crate) struct CurrentContextGuard {
    old_dc: HDC,
    old_glrc: HGLRC,
}

impl Drop for CurrentContextGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            wglMakeCurrent(self.old_dc, self.old_glrc);
        }
    }
}

impl CurrentContextGuard {
    #[inline]
    pub(crate) fn new() -> CurrentContextGuard {
        unsafe {
            CurrentContextGuard {
                old_dc: wglGetCurrentDC(),
                old_glrc: wglGetCurrentContext(),
            }
        }
    }
}
