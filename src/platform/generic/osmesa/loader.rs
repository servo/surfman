//! OpenGL function pointer loading for OSMesa.

use osmesa_sys::OSMesaGetProcAddress;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;

pub fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_name = symbol_name.as_ptr() as *const u8 as *const c_char;
        match OSMesaGetProcAddress(symbol_name) {
            Some(pointer) => pointer as *const c_void,
            None => ptr::null(),
        }
    }
}

pub(crate) fn init() {}
