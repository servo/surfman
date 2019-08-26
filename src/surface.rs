use crate::gl_context::GLFlavor;
use crate::gl_formats::Format;
use euclid::default::Size2D;
use gleam::gl::GlType;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceDescriptor {
    pub size: Size2D<i32>,
    pub format: Format,
    pub flavor: GLFlavor,
}
