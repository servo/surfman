use GLContextMethods;
use geom::{Size2D};

#[cfg(target_os="linux")]
use platform::glx::gl_context::{GLContext};

#[cfg(not(target_os="linux"))]
use platform::not_implemented::gl_context::{GLContext};

// TODO: This is (obviously I guess) not working right now
//   I'll have to scratch my head to see if it's worthy to
//   reuse rust-layers, or roll a new NativeSurface class
//
// NOTE: The GLScreenBuffer struct, if we implement correctly
//   the abstraction layer could be a generic struct that binds
//   a native surface to the primary GLContext framebuffer object,
//   implementing resizing, etc...
struct NativeSurface;

pub struct GLScreenBuffer<'a> {
    surface: Option<NativeSurface>,
    gl: &'a GLContext
}

impl<'a> GLScreenBuffer<'a> {
    fn new(gl: &'a GLContext, size: Size2D<usize>) {
        let buffer = GLScreenBuffer {
            surface: None,
            gl: gl,
        };
    }
}
