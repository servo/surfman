//! OpenGL function pointer loading.

use crate::glx;

use gl::types::GLubyte;
use libc::dlsym;
use std::ffi::CString;
use std::os::raw::c_void;

lazy_static! {
    static ref GLX_GET_PROC_ADDRESS: extern "C" fn(*const GLubyte) -> extern "C" fn() {
        unsafe {
            let function = dlsym(&b"glXGetProcAddress\0"[0]);
            assert!(!function.is_null());
            function
        }
    }
}

pub fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        (GLX_GET_PROC_ADDRESS.get())(symbol_name.as_ptr() as *const u8)
    }
}

pub(crate) fn init() {
    glx::load_with(get_proc_address);
}
