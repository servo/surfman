use geom::Size2D;
use NativeGLContextMethods;

pub struct NativeGLContext;

impl NativeGLContextMethods for NativeGLContext {
    fn create_headless(_: Size2D<i32>) -> Result<NativeGLContext, &'static str> {
        Err("Not implemented (yet)")
    }

    fn is_current(&self) -> bool {
        false
    }

    fn make_current(&self) -> Result<(), &'static str> {
        Err("Not implemented (yet)")
    }
}
