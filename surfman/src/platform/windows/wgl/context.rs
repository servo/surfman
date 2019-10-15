// surfman/src/platform/windows/wgl/context.rs
//
//! Wrapper for WGL contexts on Windows.

use crate::context::CREATE_CONTEXT_MUTEX;
use crate::{ContextAttributeFlags, ContextAttributes, ContextID, Error};
use crate::{GLVersion, WindowingApiError};
use super::adapter::Adapter;
use super::device::{DCGuard, Device, HiddenWindow};
use super::surface::{Surface, SurfaceType, Win32Objects};

use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl::{self, Gl};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::thread;
use winapi::shared::minwindef::{self, BOOL, FALSE, FLOAT, LPARAM, LPVOID, LRESULT, UINT};
use winapi::shared::minwindef::{WORD, WPARAM};
use winapi::shared::ntdef::{HANDLE, LPCSTR};
use winapi::shared::windef::{HBRUSH, HDC, HGLRC, HWND};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::libloaderapi;
use winapi::um::wingdi::{self, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE};
use winapi::um::wingdi::{PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR};
use winapi::um::wingdi::{wglCreateContext, wglDeleteContext, wglGetCurrentContext, wglGetCurrentDC, wglGetProcAddress, wglMakeCurrent};
use winapi::um::winuser::{self, COLOR_BACKGROUND, CS_OWNDC, MSG, WM_CREATE, WM_DESTROY};
use winapi::um::winuser::{WNDCLASSA, WS_OVERLAPPEDWINDOW, WS_VISIBLE};

const WGL_DRAW_TO_WINDOW_ARB:        GLenum = 0x2001;
const WGL_ACCELERATION_ARB:          GLenum = 0x2003;
const WGL_SUPPORT_OPENGL_ARB:        GLenum = 0x2010;
const WGL_DOUBLE_BUFFER_ARB:         GLenum = 0x2011;
const WGL_PIXEL_TYPE_ARB:            GLenum = 0x2013;
const WGL_COLOR_BITS_ARB:            GLenum = 0x2014;
const WGL_ALPHA_BITS_ARB:            GLenum = 0x201b;
const WGL_DEPTH_BITS_ARB:            GLenum = 0x2022;
const WGL_STENCIL_BITS_ARB:          GLenum = 0x2023;
const WGL_FULL_ACCELERATION_ARB:     GLenum = 0x2027;
const WGL_TYPE_RGBA_ARB:             GLenum = 0x202b;
const WGL_CONTEXT_MAJOR_VERSION_ARB: GLenum = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: GLenum = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB:  GLenum = 0x9126;

const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: GLenum = 0x00000001;

#[allow(non_snake_case)]
#[derive(Default)]
pub(crate) struct WGLExtensionFunctions {
    CreateContextAttribsARB: Option<unsafe extern "C" fn(hDC: HDC,
                                                         shareContext: HGLRC,
                                                         attribList: *const c_int)
                                                         -> HGLRC>,
    GetExtensionsStringARB: Option<unsafe extern "C" fn(hdc: HDC) -> *const c_char>,
    pub(crate) pixel_format_functions: Option<WGLPixelFormatExtensionFunctions>,
    pub(crate) dx_interop_functions: Option<WGLDXInteropExtensionFunctions>,
}

#[allow(non_snake_case)]
pub(crate) struct WGLPixelFormatExtensionFunctions {
    ChoosePixelFormatARB: unsafe extern "C" fn(hdc: HDC,
                                               piAttribIList: *const c_int,
                                               pfAttribFList: *const FLOAT,
                                               nMaxFormats: UINT,
                                               piFormats: *mut c_int,
                                               nNumFormats: *mut UINT)
                                               -> BOOL,
    GetPixelFormatAttribivARB: unsafe extern "C" fn(hdc: HDC,
                                                    iPixelFormat: c_int,
                                                    iLayerPlane: c_int,
                                                    nAttributes: UINT,
                                                    piAttributes: *const c_int,
                                                    piValues: *mut c_int)
                                                    -> BOOL,
}

#[allow(non_snake_case)]
pub(crate) struct WGLDXInteropExtensionFunctions {
    pub(crate) DXCloseDeviceNV: unsafe extern "C" fn(hDevice: HANDLE) -> BOOL,
    pub(crate) DXLockObjectsNV: unsafe extern "C" fn(hDevice: HANDLE,
                                                     count: GLint,
                                                     hObjects: *mut HANDLE)
                                                     -> BOOL,
    pub(crate) DXOpenDeviceNV: unsafe extern "C" fn(dxDevice: *mut c_void) -> HANDLE,
    pub(crate) DXRegisterObjectNV: unsafe extern "C" fn(hDevice: HANDLE,
                                                        dxResource: *mut c_void,
                                                        name: GLuint,
                                                        object_type: GLenum,
                                                        access: GLenum)
                                                        -> HANDLE,
    pub(crate) DXSetResourceShareHandleNV: unsafe extern "C" fn(dxResource: *mut c_void,
                                                                shareHandle: HANDLE)
                                                                -> BOOL,
    DXUnlockObjectsNV: unsafe extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE)
                                            -> BOOL,
    pub(crate) DXUnregisterObjectNV: unsafe extern "C" fn(hDevice: HANDLE, hObject: HANDLE)
                                                          -> BOOL,
}

#[derive(Clone)]
pub struct ContextDescriptor {
    pixel_format: c_int,
    gl_version: GLVersion,
}

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    pub(crate) gl: Gl,
    hidden_window: Option<HiddenWindow>,
    framebuffer: Framebuffer,
}

pub(crate) trait NativeContext {
    fn glrc(&self) -> HGLRC;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

lazy_static! {
    pub(crate) static ref WGL_EXTENSION_FUNCTIONS: WGLExtensionFunctions = {
        thread::spawn(extension_loader_thread).join().unwrap()
    };
}

enum Framebuffer {
    None,
    External { dc: HDC },
    Surface(Surface),
}

impl Device {
    #[allow(non_snake_case)]
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;
        let alpha_bits   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_bits   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_bits = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        let attrib_i_list = [
            WGL_DRAW_TO_WINDOW_ARB as c_int, gl::TRUE as c_int,
            WGL_SUPPORT_OPENGL_ARB as c_int, gl::TRUE as c_int,
            WGL_DOUBLE_BUFFER_ARB as c_int,  gl::TRUE as c_int,
            WGL_PIXEL_TYPE_ARB as c_int,     WGL_TYPE_RGBA_ARB as c_int,
            WGL_ACCELERATION_ARB as c_int,   WGL_FULL_ACCELERATION_ARB as c_int,
            WGL_COLOR_BITS_ARB as c_int,     32,
            WGL_ALPHA_BITS_ARB as c_int,     alpha_bits,
            WGL_DEPTH_BITS_ARB as c_int,     depth_bits,
            WGL_STENCIL_BITS_ARB as c_int,   stencil_bits,
            0,
        ];

        let wglChoosePixelFormatARB = match WGL_EXTENSION_FUNCTIONS.pixel_format_functions {
            None => return Err(Error::RequiredExtensionUnavailable),
            Some(ref pixel_format_functions) => pixel_format_functions.ChoosePixelFormatARB,
        };

        let hidden_window_dc = self.hidden_window.get_dc();
        unsafe {
            let (mut pixel_format, mut pixel_format_count) = (0, 0);
            let ok = wglChoosePixelFormatARB(hidden_window_dc.dc,
                                             attrib_i_list.as_ptr(),
                                             ptr::null(),
                                             1,
                                             &mut pixel_format,
                                             &mut pixel_format_count);
            if ok == FALSE {
                return Err(Error::PixelFormatSelectionFailed(WindowingApiError::Failed));
            }
            if pixel_format_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            Ok(ContextDescriptor { pixel_format, gl_version: attributes.version })
        }
    }

    /// Opens the device and context corresponding to the current WGL context.
    ///
    /// The native context is not retained, as there is no way to do this in the WGL API. It is
    /// the caller's responsibility to keep it alive for the duration of this context. Be careful
    /// when using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library; for example, by Glutin. It's legal to use this method to wrap a context rendering
    /// to any target: either a window or a pbuffer. The target is opaque to `surfman`; the
    /// library will not modify or try to detect the render target. This means that any of the
    /// methods that query or replace the surface—e.g. `replace_context_surface`—will fail if
    /// called with a context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let device = Device::new(&Adapter::default()?)?;
        unsafe {
            let context = Context {
                native_context: Box::new(UnsafeGLRCRef { glrc: wglGetCurrentContext() }),
                id: *next_context_id,
                gl: Gl::load_with(get_proc_address),
                hidden_window: None,
                framebuffer: Framebuffer::External { dc: wglGetCurrentDC() },
            };
            next_context_id.0 += 1;

            Ok((device, context))
        }
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor, surface_type: &SurfaceType)
                          -> Result<Context, Error> {
        let wglCreateContextAttribsARB = match WGL_EXTENSION_FUNCTIONS.CreateContextAttribsARB {
            None => return Err(Error::RequiredExtensionUnavailable),
            Some(wglCreateContextAttribsARB) => wglCreateContextAttribsARB,
        };

        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        unsafe {
            let (glrc, gl);

            // Get a suitable DC.
            let hidden_window = match *surface_type {
                SurfaceType::Widget { ref native_widget } => None,
                SurfaceType::Generic { .. } => Some(HiddenWindow::new()),
            };

            {
                let hidden_window_dc = match hidden_window {
                    None => None,
                    Some(ref hidden_window) => Some(hidden_window.get_dc()),
                };
                let dc = match *surface_type {
                    SurfaceType::Widget { ref native_widget } => {
                        winuser::GetDC(native_widget.window_handle)
                    }
                    SurfaceType::Generic { .. } => hidden_window_dc.as_ref().unwrap().dc,
                };

                // Set the pixel format on the DC.
                let mut pixel_format_descriptor = mem::zeroed();
                let pixel_format_count =
                    wingdi::DescribePixelFormat(dc,
                                                descriptor.pixel_format,
                                                mem::size_of::<PIXELFORMATDESCRIPTOR>() as UINT,
                                                &mut pixel_format_descriptor);
                assert_ne!(pixel_format_count, 0);
                let ok = wingdi::SetPixelFormat(dc,
                                                descriptor.pixel_format,
                                                &mut pixel_format_descriptor);
                assert_ne!(ok, FALSE);

                // Make the context.
                let wgl_attributes = [
                    WGL_CONTEXT_MAJOR_VERSION_ARB as c_int, descriptor.gl_version.major as c_int,
                    WGL_CONTEXT_MINOR_VERSION_ARB as c_int, descriptor.gl_version.minor as c_int,
                    WGL_CONTEXT_PROFILE_MASK_ARB as c_int,
                        WGL_CONTEXT_CORE_PROFILE_BIT_ARB as c_int,
                    0,
                ];
                glrc = wglCreateContextAttribsARB(dc, ptr::null_mut(), wgl_attributes.as_ptr());
                if glrc.is_null() {
                    return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
                }

                // Temporarily make the context current.
                let _guard = CurrentContextGuard::new();
                let ok = wglMakeCurrent(dc, glrc);
                assert_ne!(ok, FALSE);

                // Load the GL functions.
                gl = Gl::load_with(get_proc_address);
            }

            // Create the initial context.
            let mut context = Context {
                native_context: Box::new(OwnedGLRC { glrc }),
                id: *next_context_id,
                gl,
                hidden_window,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;

            // Build the initial framebuffer.
            let surface = self.create_surface(&context, surface_type)?;
            self.lock_surface(&surface);
            context.framebuffer = Framebuffer::Surface(surface);
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Framebuffer::Surface(surface) = mem::replace(&mut context.framebuffer,
                                                            Framebuffer::None) {
            self.destroy_surface(context, surface)?;
        }

        unsafe {
            context.native_context.destroy();
        }

        Ok(())
    }

    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let dc_guard = self.get_context_dc(context);
            let pixel_format = wingdi::GetPixelFormat(dc_guard.dc);

            let _guard = self.temporarily_make_context_current(context);
            let version_string = context.gl.GetString(gl::VERSION) as *const c_char;
            let version_string = CStr::from_ptr(version_string).to_string_lossy();
            let mut version_string_iter = version_string.split(".");
            let major_version: u8 =
                version_string_iter.next()
                                   .expect("Where's the major GL version?")
                                   .parse()
                                   .expect("Couldn't parse the major GL version!");
            let minor_version: u8 =
                version_string_iter.next()
                                   .expect("Where's the minor GL version?")
                                   .parse()
                                   .expect("Couldn't parse the minor GL version!");
            ContextDescriptor {
                pixel_format,
                gl_version: GLVersion::new(major_version, minor_version),
            }
        }
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        let wglGetPixelFormatAttribivARB =
            WGL_EXTENSION_FUNCTIONS.pixel_format_functions
                                   .as_ref()
                                   .expect("How did you make a context descriptor without \
                                            pixel format extensions?")
                                   .GetPixelFormatAttribivARB;

        let dc_guard = self.hidden_window.get_dc();

        unsafe {
            let attrib_name_i_list = [
                WGL_ALPHA_BITS_ARB as c_int,
                WGL_DEPTH_BITS_ARB as c_int,
                WGL_STENCIL_BITS_ARB as c_int,
            ];
            let mut attrib_value_i_list = [0; 3];
            let ok = wglGetPixelFormatAttribivARB(dc_guard.dc,
                                                  context_descriptor.pixel_format,
                                                  0,
                                                  attrib_name_i_list.len() as UINT,
                                                  attrib_name_i_list.as_ptr(),
                                                  attrib_value_i_list.as_mut_ptr());
            assert_ne!(ok, FALSE);
            let (alpha_bits, depth_bits, stencil_bits) =
                (attrib_value_i_list[0], attrib_value_i_list[1], attrib_value_i_list[2]);

            let mut attributes = ContextAttributes {
                version: context_descriptor.gl_version,
                flags: ContextAttributeFlags::empty(),
            };
            if alpha_bits > 0 {
                attributes.flags.insert(ContextAttributeFlags::ALPHA);
            }
            if depth_bits > 0 {
                attributes.flags.insert(ContextAttributeFlags::DEPTH);
            }
            if stencil_bits > 0 {
                attributes.flags.insert(ContextAttributeFlags::STENCIL);
            }

            attributes
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if let Framebuffer::External { .. } = context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        unsafe {
            let is_current = self.context_is_current(context);

            let old_surface = self.release_surface(context).expect("Where's our surface?");
            self.attach_surface(context, new_surface);

            if is_current {
                // We need to make ourselves current again, because the surface changed.
                self.make_context_current(context)?;
            }

            Ok(old_surface)
        }
    }

    pub(crate) fn temporarily_bind_framebuffer<'a>(&self,
                                                   context: &'a Context,
                                                   framebuffer: GLuint)
                                                   -> FramebufferGuard<'a> {
        unsafe {
            let guard = FramebufferGuard::new(context);
            context.gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
            guard
        }
    }

    pub(crate) fn temporarily_make_context_current(&self, context: &Context)
                                                   -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let dc_guard = self.get_context_dc(context);
            let ok = wglMakeCurrent(dc_guard.dc, context.native_context.glrc());
            if ok != FALSE {
                Ok(())
            } else {
                Err(Error::MakeCurrentFailed(WindowingApiError::Failed))
            }
        }
    }

    #[inline]
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            let ok = wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
            if ok != FALSE {
                Ok(())
            } else {
                Err(Error::MakeCurrentFailed(WindowingApiError::Failed))
            }
        }
    }

    #[inline]
    fn context_is_current(&self, context: &Context) -> bool {
        unsafe {
            wglGetCurrentContext() == context.native_context.glrc()
        }
    }

    fn attach_surface(&self, context: &mut Context, surface: Surface) {
        match context.framebuffer {
            Framebuffer::None => context.framebuffer = Framebuffer::Surface(surface),
            _ => panic!("Tried to attach a surface, but there was already a surface present!"),
        }
    }

    fn release_surface(&self, context: &mut Context) -> Option<Surface> {
        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => Some(surface),
            Framebuffer::None | Framebuffer::External { .. } => None,
        }
    }

    fn get_context_dc<'a>(&self, context: &'a Context) -> DCGuard<'a> {
        unsafe {
            match context.framebuffer {
                Framebuffer::None => unreachable!(),
                Framebuffer::External { dc } => DCGuard::new(dc, None),
                Framebuffer::Surface(ref surface) => {
                    match surface.win32_objects {
                        Win32Objects::Widget { window_handle } => {
                            DCGuard::new(winuser::GetDC(window_handle), Some(window_handle))
                        }
                        Win32Objects::Texture { .. } => {
                            context.hidden_window.as_ref().unwrap().get_dc()
                        }
                    }
                }
            }
        }
    }
}

fn extension_loader_thread() -> WGLExtensionFunctions {
    unsafe {
        let instance = libloaderapi::GetModuleHandleA(ptr::null_mut());
        let window_class_name = &b"SurfmanFalseWindow\0"[0] as *const u8 as LPCSTR;
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
        let window = winuser::CreateWindowExA(
            0,
            window_class_atom as LPCSTR,
            window_class_name,
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
                let pixel_format = wingdi::ChoosePixelFormat(dc, &pixel_format_descriptor);
                assert_ne!(pixel_format, 0);
                let mut ok = wingdi::SetPixelFormat(dc, pixel_format, &pixel_format_descriptor);
                assert_ne!(ok, FALSE);
                let gl_context = wglCreateContext(dc);
                assert!(!gl_context.is_null());
                ok = wglMakeCurrent(dc, gl_context);
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
                        (*wgl_extension_functions).pixel_format_functions =
                            Some(WGLPixelFormatExtensionFunctions {
                                ChoosePixelFormatARB: mem::transmute(wglGetProcAddress(
                                    &b"wglChoosePixelFormatARB\0"[0] as *const u8 as LPCSTR)),
                                GetPixelFormatAttribivARB: mem::transmute(wglGetProcAddress(
                                    &b"wglGetPixelFormatAttribivARB\0"[0] as *const u8 as LPCSTR)),
                            });
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

struct OwnedGLRC {
    glrc: HGLRC,
}

impl NativeContext for OwnedGLRC {
    #[inline]
    fn glrc(&self) -> HGLRC {
        debug_assert!(!self.is_destroyed());
        self.glrc
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.glrc.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        if wglGetCurrentContext() == self.glrc {
            wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
        }
        wglDeleteContext(self.glrc);
        self.glrc = ptr::null_mut();
    }
}

struct UnsafeGLRCRef {
    glrc: HGLRC,
}

impl NativeContext for UnsafeGLRCRef {
    #[inline]
    fn glrc(&self) -> HGLRC {
        debug_assert!(!self.is_destroyed());
        self.glrc
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.glrc.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.glrc = ptr::null_mut();
    }
}

#[must_use]
pub(crate) struct FramebufferGuard<'a> {
    context: &'a Context,
    old_read_framebuffer: GLuint,
    old_draw_framebuffer: GLuint,
}

impl<'a> Drop for FramebufferGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.context.gl.BindFramebuffer(gl::READ_FRAMEBUFFER, self.old_read_framebuffer);
            self.context.gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.old_draw_framebuffer);
        }
    }
}

impl<'a> FramebufferGuard<'a> {
    fn new(context: &'a Context) -> FramebufferGuard<'a> {
        unsafe {
            let (mut current_draw_framebuffer, mut current_read_framebuffer) = (0, 0);
            context.gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut current_draw_framebuffer);
            context.gl.GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut current_read_framebuffer);

            FramebufferGuard {
                context,
                old_draw_framebuffer: current_draw_framebuffer as GLuint,
                old_read_framebuffer: current_read_framebuffer as GLuint,
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
    fn new() -> CurrentContextGuard {
        unsafe {
            CurrentContextGuard {
                old_dc: wglGetCurrentDC(),
                old_glrc: wglGetCurrentContext(),
            }
        }
    }
}

fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        wglGetProcAddress(symbol_name.as_ptr() as *const u8 as LPCSTR) as *const c_void
    }
}
