use NativeGLContextMethods;

pub struct NativeGLContext;

#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeGraphicsMetadata;

impl NativeGLContextMethods for NativeGLContext {
    fn get_proc_address(_addr: &str) -> *const () {
        0 as *const ()
    }

    fn create_headless() -> Result<NativeGLContext, &'static str> {
        Err("Not implemented (yet)")
    }

    fn is_current(&self) -> bool {
        false
    }

    fn make_current(&self) -> Result<(), &'static str> {
        Err("Not implemented (yet)")
    }

    fn unbind(&self) -> Result<(), &'static str> {
        unimplemented!()
    }

    #[cfg(feature="texture_surface")]
    fn get_metadata(&self) -> NativeGraphicsMetadata {
        unimplemented!()
    }
}
