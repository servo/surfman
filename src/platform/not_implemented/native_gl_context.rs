use geom::Size2D;
use NativeGLContextMethods;

pub struct GLContext;

impl GLContextMethods for GLContext {
    fn create_headless(_: Size2D<i32>) -> Result<GLContext, &'static str> {
        Err("Not implemented (yet)")
    }

    fn is_current() {
        false
    }

    fn make_current(&self) -> Result<(), &'static str> {
        Err("Not implemented (yet)")
    }
}
