//! Information related to hardware surfaces.

use crate::{ContextAttributes, ContextAttributeFlags, GLFlavor};
use euclid::default::Size2D;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceId(pub usize);

impl Display for SurfaceId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceDescriptor {
    pub size: Size2D<i32>,
    pub format: SurfaceFormat,
    pub flavor: GLFlavor,
}

impl SurfaceDescriptor {
    #[inline]
    pub fn from_context_attributes_and_size(attributes: &ContextAttributes, size: &Size2D<i32>)
                                            -> SurfaceDescriptor {
        SurfaceDescriptor {
            size: *size,
            format: if attributes.flags.contains(ContextAttributeFlags::ALPHA) {
                SurfaceFormat::RGBA8
            } else {
                SurfaceFormat::RGB8
            },
            flavor: attributes.flavor,
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
