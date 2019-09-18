//! OpenGL function pointer loading.

use core_foundation::base::TCFType;
use core_foundation::bundle::CFBundleRef;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;
use std::os::raw::c_void;
use std::str::FromStr;

static OPENGL_FRAMEWORK_IDENTIFIER: &'static str = "com.apple.opengl";

thread_local! {
    static OPENGL_FRAMEWORK: CFBundleRef = {
        unsafe {
            let framework_identifier: CFString =
                FromStr::from_str(OPENGL_FRAMEWORK_IDENTIFIER).unwrap();
            let framework =
                CFBundleGetBundleWithIdentifier(framework_identifier.as_concrete_TypeRef());
            assert!(!framework.is_null());
            framework
        }
    };
}

pub(crate) fn init() {}

pub fn get_proc_address(symbol_name: &str) -> *const c_void {
    OPENGL_FRAMEWORK.with(|framework| {
        unsafe {
            let symbol_name: CFString = FromStr::from_str(symbol_name).unwrap();
            CFBundleGetFunctionPointerForName(*framework, symbol_name.as_concrete_TypeRef())
        }
    })
}
