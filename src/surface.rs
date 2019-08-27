use crate::gl_context::GLFlavor;
use crate::gl_formats::Format;
use euclid::default::Size2D;
use gleam::gl::GlType;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceDescriptor {
    pub size: Size2D<i32>,
    pub format: SurfaceFormat,
    pub flavor: GLFlavor,
}

// All supported color formats for offscreen rendering.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SurfaceFormat {
    RGBA8,
    RGB8,
}

impl SurfaceFormat {
    #[inline]
    pub fn has_alpha(self) -> bool {
        self == SurfaceFormat::RGBA
    }
}
