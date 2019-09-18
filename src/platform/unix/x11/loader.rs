//! OpenGL function pointer loading.

use crate::glx;

use gl::types::GLubyte;
use libc::{RTLD_DEFAULT, dlsym};
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};

lazy_static! {
    static ref GLX_GET_PROC_ADDRESS: extern "C" fn(*const GLubyte) -> *mut c_void = {
        unsafe {
            let symbol = &b"glXGetProcAddress\0"[0] as *const u8 as *const i8;
            let function = dlsym(RTLD_DEFAULT, symbol);
            assert!(!function.is_null());
            mem::transmute(function)
        }
    };
}

pub fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        (*GLX_GET_PROC_ADDRESS)(symbol_name.as_ptr() as *const u8) as *const c_void
    }
}

pub(crate) fn init() {
    glx::load_with(get_proc_address);
}
