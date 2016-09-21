use platform::NativeGLContextMethods;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::sync::{Once, ONCE_INIT};

use winapi;
use user32;
use kernel32;
use super::wgl;
use super::wgl_attributes::*;
use gleam::gl;

/// Wrapper to satisfy `Sync`.
struct HMODULEWrapper(winapi::HMODULE);
unsafe impl Sync for HMODULEWrapper {}

lazy_static! {
    static ref GL_LIB: Option<HMODULEWrapper>  = {
        let p = unsafe{kernel32::LoadLibraryA(b"opengl32.dll\0".as_ptr() as *const _)};
        if p.is_null() {
            error!("WGL: opengl32.dll not found!");
            None
        } else {
            debug!("WGL: opengl32.dll loaded!");
            Some(HMODULEWrapper(p))
        }
    };
}

static LOAD_GL: Once = ONCE_INIT;
pub fn load_gl() {
    LOAD_GL.call_once(|| {
        gl::load_with(|s| NativeGLContext::get_proc_address(s) as *const _);
    });
}

pub struct NativeGLContext {
    render_ctx: winapi::HGLRC,
    device_ctx: winapi::HDC,
    weak: bool,
}

impl Drop for NativeGLContext {
    fn drop(&mut self) {
        unsafe {
            if !self.weak {
                wgl::DeleteContext(self.render_ctx as *const _);
                let window = user32::WindowFromDC(self.device_ctx);
                user32::ReleaseDC(window, self.device_ctx);
                user32::DestroyWindow(window);
            }
        }
    }
}

pub struct NativeGLContextHandle(winapi::HGLRC, winapi::HDC);
unsafe impl Send for NativeGLContextHandle {}

impl NativeGLContextMethods for NativeGLContext {
    type Handle = NativeGLContextHandle;

    fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            let p = wgl::GetProcAddress(addr) as *const _;
            if !p.is_null() {
                return p;
            }
            match *GL_LIB {
                Some(ref lib) => kernel32::GetProcAddress(lib.0, addr) as *const _,
                None => ptr::null_mut(),
            }
        }

    }

    fn create_shared(with: Option<&Self::Handle>) -> Result<NativeGLContext, &'static str> {
        let render_ctx = match with {
            Some(ref handle) => handle.0,
            None => ptr::null_mut(),
        };
        match unsafe { super::utils::create_offscreen(render_ctx, &WGLAttributes::default()) } {
            Ok(ref res) => {
                // wglGetProcAddress only works in the presence of a valid GL context
                // OpenGL functions must be loaded after the first context is created
                let ctx = NativeGLContext {
                    render_ctx: res.0,
                    device_ctx: res.1,
                    weak: false,
                };
                ctx.make_current().unwrap();
                load_gl();
                Ok(ctx)
            }
            Err(s) => {
                error!("WGL: {}", s);
                Err("Error creating WGL context")
            }
        }
    }

    fn is_current(&self) -> bool {
        unsafe { wgl::GetCurrentContext() == self.render_ctx as *const c_void }
    }

    fn current() -> Option<Self> {
        if let Some(handle) = Self::current_handle() {
            Some(NativeGLContext {
                render_ctx: handle.0,
                device_ctx: handle.1,
                weak: true,
            })
        } else {
            None
        }
    }

    fn current_handle() -> Option<Self::Handle> {
        let ctx = unsafe { wgl::GetCurrentContext() };
        let hdc = unsafe { wgl::GetCurrentDC() };
        if ctx.is_null() || hdc.is_null() {
            None
        } else {
            Some(NativeGLContextHandle(ctx as winapi::HGLRC, hdc as winapi::HDC))
        }
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if wgl::MakeCurrent(self.device_ctx as *const _, self.render_ctx as *const _) != 0 {
                Ok(())
            } else {
                Err("wgl::makeCurrent failed")
            }
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        unsafe {
            if self.is_current() && wgl::MakeCurrent(ptr::null_mut(), ptr::null_mut()) == 0 {
                Err("gwl::MakeCurrent (on unbind)")
            } else {
                Ok(())
            }
        }
    }

    fn handle(&self) -> Self::Handle {
        NativeGLContextHandle(self.render_ctx, self.device_ctx)
    }
}
