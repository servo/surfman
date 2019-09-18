//! OpenGL function pointer loading.

use std::os::raw::c_void;

pub fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        eglGetProcAddress(symbol_name.as_ptr() as *const u8)
    }
}

pub(crate) fn init() {}
