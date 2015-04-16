use geom::Size2D;
use common_methods::GLContextMethods;
use gl_context_attributes::GLContextAttributes;

pub struct GLContext;

impl GLContextMethods for GLContext {
    fn create_headless(_: Size2D<i32>) -> Result<GLContext, &'static str> {
        Err("Not implemented (yet)")
    }

    fn create_offscreen(_: Size2D<i32>, _: GLContextAttributes) -> Result<GLContext, &'static str> {
        Err("Not implemented (yet)")
    }

    fn make_current(&self) -> Result<(), &'static str> {
        Err("Not implemented (yet)")
    }
}
