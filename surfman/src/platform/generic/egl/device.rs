// surfman/surfman/src/platform/generic/egl/device.rs
//
//! Functionality common to backends using EGL displays.

use crate::egl::Egl;

use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};

#[cfg(not(target_os = "windows"))]
use libc::{dlopen, dlsym, RTLD_LAZY};
#[cfg(target_os = "windows")]
use winapi::shared::minwindef::HMODULE;
#[cfg(target_os = "windows")]
use winapi::shared::ntdef::LPCSTR;
#[cfg(target_os = "windows")]
use winapi::um::libloaderapi;

thread_local! {
    pub static EGL_FUNCTIONS: Egl = Egl::load_with(get_proc_address);
}

#[cfg(target_os = "windows")]
lazy_static! {
    static ref EGL_LIBRARY: EGLLibraryWrapper = {
        unsafe {
            let module = libloaderapi::LoadLibraryA(&b"libEGL.dll\0"[0] as *const u8 as LPCSTR);
            EGLLibraryWrapper(module)
        }
    };
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
lazy_static! {
    static ref EGL_LIBRARY: EGLLibraryWrapper = {
        unsafe {
            EGLLibraryWrapper(dlopen(
                &b"libEGL.so\0"[0] as *const u8 as *const _,
                RTLD_LAZY,
            ))
        }
    };
}

#[cfg(target_os = "windows")]
struct EGLLibraryWrapper(HMODULE);
#[cfg(not(target_os = "windows"))]
struct EGLLibraryWrapper(*mut c_void);

unsafe impl Send for EGLLibraryWrapper {}
unsafe impl Sync for EGLLibraryWrapper {}

#[cfg(target_os = "windows")]
fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_ptr = symbol_name.as_ptr() as *const u8 as LPCSTR;
        libloaderapi::GetProcAddress(EGL_LIBRARY.0, symbol_ptr) as *const c_void
    }
}

#[cfg(not(target_os = "windows"))]
fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_ptr = symbol_name.as_ptr() as *const u8 as *const c_char;
        dlsym(EGL_LIBRARY.0, symbol_ptr) as *const c_void
    }
}

pub(crate) unsafe fn lookup_egl_extension(name: &'static [u8]) -> *mut c_void {
    EGL_FUNCTIONS
        .with(|egl| mem::transmute(egl.GetProcAddress(&name[0] as *const u8 as *const c_char)))
}
