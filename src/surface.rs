//! Information related to hardware surfaces.

use crate::{ContextAttributes, GLFlavor, GLInfo};
use euclid::default::Size2D;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceDescriptor {
    pub size: Size2D<i32>,
    pub format: SurfaceFormat,
    pub flavor: GLFlavor,
}

impl SurfaceDescriptor {
    #[inline]
    pub fn from_gl_info_and_size(info: &GLInfo, size: &Size2D<i32>) -> SurfaceDescriptor {
        SurfaceDescriptor {
            size: *size,
            format: if info.attributes.contains(ContextAttributes::ALPHA) {
                SurfaceFormat::RGBA8
            } else {
                SurfaceFormat::RGB8
            },
            flavor: info.flavor,
        }
    }
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
        match self {
            SurfaceFormat::RGBA8 => true,
            SurfaceFormat::RGB8 => false,
        }
    }
}
