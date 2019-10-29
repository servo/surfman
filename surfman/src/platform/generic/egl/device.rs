// surfman/surfman/src/platform/generic/egl/device.rs
//
//! Functionality common to backends using EGL displays.

use crate::egl::types::EGLDisplay;
use crate::egl::{self, Egl};

use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};

#[cfg(target_os = "windows")]
use winapi::um::libloaderapi;
#[cfg(not(target_os = "windows"))]
use libc::{RTLD_LAZY, dlopen, dlsym};

thread_local! {
    pub static EGL_FUNCTIONS: Egl = Egl::load_with(get_proc_address);
}

#[cfg(target_os = "windows")]
lazy_static! {
    static ref EGL_LIBRARY: HMODULE = {
        unsafe {
            libloaderapi::LoadLibraryA(&b"libEGL.dll\0"[0] as *const u8 as LPCSTR)
        }
    };
}

#[cfg(not(target_os = "windows"))]
lazy_static! {
    static ref EGL_LIBRARY: EGLLibraryWrapper = {
        unsafe {
            EGLLibraryWrapper(dlopen(&b"libEGL.so\0"[0] as *const u8 as *const i8, RTLD_LAZY))
        }
    };
}

#[cfg(not(target_os = "windows"))]
struct EGLLibraryWrapper(*mut c_void);
#[cfg(not(target_os = "windows"))]
unsafe impl Send for EGLLibraryWrapper {}
#[cfg(not(target_os = "windows"))]
unsafe impl Sync for EGLLibraryWrapper {}

pub(crate) trait NativeDisplay {
    fn egl_display(&self) -> EGLDisplay;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

pub(crate) struct OwnedEGLDisplay {
    pub(crate) egl_display: EGLDisplay,
}

impl NativeDisplay for OwnedEGLDisplay {
    #[inline]
    fn egl_display(&self) -> EGLDisplay {
        debug_assert!(!self.is_destroyed());
        self.egl_display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_display == egl::NO_DISPLAY
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());

        EGL_FUNCTIONS.with(|egl| {
            let result = egl.Terminate(self.egl_display);
            assert_ne!(result, egl::FALSE);
            self.egl_display = egl::NO_DISPLAY;
        })
    }
}

pub(crate) struct UnsafeEGLDisplayRef {
    egl_display: EGLDisplay,
}

impl NativeDisplay for UnsafeEGLDisplayRef {
    #[inline]
    fn egl_display(&self) -> EGLDisplay {
        debug_assert!(!self.is_destroyed());
        self.egl_display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_display == egl::NO_DISPLAY
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.egl_display = egl::NO_DISPLAY;
    }
}

#[cfg(target_os = "windows")]
fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_ptr = symbol_name.as_ptr() as *const u8 as LPCSTR;
        libloaderapi::GetProcAddress(*EGL_LIBRARY, symbol_ptr) as *const c_void
    }
}

#[cfg(not(target_os = "windows"))]
fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_ptr = symbol_name.as_ptr() as *const u8 as *const i8;
        dlsym(EGL_LIBRARY.0, symbol_ptr) as *const c_void
    }
}

pub(crate) unsafe fn lookup_egl_extension(name: &'static [u8]) -> *mut c_void {
    EGL_FUNCTIONS.with(|egl| {
        mem::transmute(egl.GetProcAddress(&name[0] as *const u8 as *const c_char))
    })
}
