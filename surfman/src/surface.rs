// surfman/surfman/src/surface.rs 
//
//! Declarations common to all surfaces.

use crate::gl::types::GLuint;
use crate::ContextID;

use euclid::default::Size2D;
use std::fmt::{self, Display, Formatter};

pub struct SurfaceInfo {
    /// The ID of this surface.
    /// 
    /// This is guaranteed to be unique among all currently-allocated surfaces.
    pub id: SurfaceID,

    /// The size of this surface, in device pixels.
    pub size: Size2D<i32>,

    /// The ID of the context that this surface is associated with.
    pub context_id: ContextID;

    /// The OpenGL framebuffer object that can be used to render to (or read from) this surface.
    ///
    /// This framebuffer object is only valid if the surface is currently attached to its
    /// associated context. Do not assume that this value necessarily remains the same across the
    /// lifetime of the surface; this value may change whenever the surface is attached to a
    /// context.
    /// 
    /// This value can be zero, in which case this surface is represented by the default
    /// framebuffer.
    pub framebuffer_object: GLuint;
}

// The default framebuffer for a context.
#[allow(dead_code)]
pub(crate) enum Framebuffer<S> {
    // No framebuffer has been attached to the context.
    None,
    // The context is externally-managed.
    External,
    // The context renders to a surface.
    Surface(S),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceID(pub usize);

impl Display for SurfaceID {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SurfaceAccess {
    /// The surface data is accessible by the GPU only.
    /// 
    /// The `lock_surface_data()` method will return the `SurfaceDataInaccessible` error when
    /// called on this surface.
    GPUOnly,

    /// The surface data is accessible by the GPU and CPU.
    GPUCPU,

    /// The surface data is accessible by the GPU and CPU, and the CPU will send surface data over
    /// the bus to the GPU using write-combining if available.
    /// 
    /// Specifically, what this means is that data transfer will be optimized for the following
    /// patterns:
    /// 
    /// 1. Writing, not reading.
    /// 
    /// 2. Writing sequentially, filling every byte in a range.
    /// 
    /// This flag has no effect on correctness (at least on x86), but not following the rules
    /// above may result in severe performance consequences.
    /// 
    /// The driver is free to treat this as identical to `GPUCPU`.
    GPUCPUWriteCombined,
}

pub enum SurfaceType<NativeWidget> {
    Generic { size: Size2D<i32> },
    Widget { native_widget: NativeWidget },
}

impl SurfaceAccess {
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn cpu_access_allowed(self) -> bool {
        match self {
            SurfaceAccess::GPUOnly => false,
            SurfaceAccess::GPUCPU | SurfaceAccess::GPUCPUWriteCombined => true,
        }
    }
}
