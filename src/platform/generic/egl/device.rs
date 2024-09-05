// surfman/surfman/src/platform/generic/egl/device.rs
//
//! Functionality common to backends using EGL displays.

use crate::egl::Egl;

#[cfg(not(target_os = "windows"))]
use libc::{dlopen, dlsym, RTLD_LAZY};
use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::c_void;
use std::sync::LazyLock;
#[cfg(target_os = "windows")]
use winapi::shared::minwindef::HMODULE;
#[cfg(target_os = "windows")]
use winapi::um::libloaderapi;

thread_local! {
    pub static EGL_FUNCTIONS: Egl = Egl::load_with(get_proc_address);
}

#[cfg(target_os = "windows")]
static EGL_LIBRARY: LazyLock<EGLLibraryWrapper> = LazyLock::new(|| unsafe {
    let module = libloaderapi::LoadLibraryA(c"libEGL.dll".as_ptr());
    EGLLibraryWrapper(module)
});

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
static EGL_LIBRARY: LazyLock<EGLLibraryWrapper> = LazyLock::new(|| {
    for soname in [c"libEGL.so.1".as_ptr(), c"libEGL.so".as_ptr()] {
        unsafe {
            let handle = dlopen(soname as *const _, RTLD_LAZY);
            if !handle.is_null() {
                return EGLLibraryWrapper(handle);
            }
        }
    }
    panic!("Unable to load the libEGL shared object");
});

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
        let symbol_ptr = symbol_name.as_ptr();
        libloaderapi::GetProcAddress(EGL_LIBRARY.0, symbol_ptr).cast()
    }
}

#[cfg(not(target_os = "windows"))]
fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_ptr = symbol_name.as_ptr();
        dlsym(EGL_LIBRARY.0, symbol_ptr).cast_const()
    }
}

pub(crate) unsafe fn lookup_egl_extension(name: &CStr) -> *mut c_void {
    EGL_FUNCTIONS.with(|egl| mem::transmute(egl.GetProcAddress(name.as_ptr())))
}
