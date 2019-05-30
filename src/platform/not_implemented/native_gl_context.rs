use gleam::gl;
use crate::NativeGLContextMethods;
use crate::GLVersion;

pub struct NativeGLContext;
pub struct NativeGLContextHandle;

impl NativeGLContextMethods for NativeGLContext {
    type Handle = NativeGLContextHandle;

    fn get_proc_address(_addr: &str) -> *const () {
        0 as *const ()
    }

    fn create_shared(_with: Option<&Self::Handle>,
                     _api_type: &gl::GlType,
                     _api_version: GLVersion) -> Result<Self, &'static str> {
        Err("Not implemented (yet)")
    }

    fn is_current(&self) -> bool {
        false
    }

    fn current() -> Option<Self> {
        None
    }

    fn current_handle() -> Option<Self::Handle> {
        None
    }

    fn make_current(&self) -> Result<(), &'static str> {
        Err("Not implemented (yet)")
    }

    fn unbind(&self) -> Result<(), &'static str> {
        unimplemented!()
    }

    fn handle(&self) -> Self::Handle {
        unimplemented!()
    }
}
