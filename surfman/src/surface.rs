// surfman/surfman/src/surface.rs
//
//! Information related to hardware surfaces.

use crate::context::ContextID;

use crate::gl::types::GLuint;
use euclid::default::Size2D;
use std::fmt::{self, Display, Formatter};

/// Various data about the surface.
pub struct SystemSurfaceInfo {
    /// The surface's size, in device pixels.
    pub size: Size2D<i32>,
    /// The ID of the surface. This should be globally unique for each currently-allocated surface.
    pub id: SurfaceID,
}

/// Various data about the surface.
pub struct SurfaceInfo {
    /// The surface's size, in device pixels.
    pub size: Size2D<i32>,
    /// The ID of the surface. This should be globally unique for each currently-allocated surface.
    pub id: SurfaceID,
    /// The ID of the context that this surface belongs to.
    pub context_id: ContextID,
    /// The OpenGL framebuffer object that can be used to render to this surface.
    ///
    /// This is only valid when the surface is actually attached to a context.
    pub framebuffer_object: GLuint,
}

// The default framebuffer for a context.
#[allow(dead_code)]
pub(crate) enum Framebuffer<S, E> {
    // No framebuffer has been attached to the context.
    None,
    // The context is externally-managed.
    External(E),
    // The context renders to a surface.
    Surface(S),
}

/// A unique ID per allocated surface.
///
/// If you destroy a surface and then create a new one, the ID may be reused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceID(pub usize);

impl Display for SurfaceID {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}

/// Specifies how and if the CPU has direct access to the surface data.
///
/// No matter what value you choose here, the CPU can always indirectly upload data to the surface
/// by, for example, drawing a full-screen quad. This enumeration simply describes whether the CPU
/// has *direct* memory access to the surface, via a slice of pixel data.
///
/// You can achieve better performance by limiting surfaces to `GPUOnly` unless you need to access
/// the data on the CPU. For surfaces marked as GPU-only, the GPU can use texture swizzling to
/// improve memory locality.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SurfaceAccess {
    /// The surface data is accessible by the GPU only.
    ///
    /// The `lock_surface_data()` method will return the `SurfaceDataInaccessible` error when
    /// called on this surface.
    ///
    /// This is typically the flag you will want to use.
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

/// Information specific to the type of surface: generic or widget.
pub enum SurfaceType<NativeWidget> {
    /// An off-screen surface that has a pixel size. Generic surfaces can sometimes be shown on
    /// screen using platform-specific APIs, but `surfman` itself provides no way to draw their
    /// contents on screen. Only generic surfaces can be bound to textures.
    Generic {
        /// The size of the surface.
        ///
        /// For HiDPI screens, this is a physical size, not a logical size.
        size: Size2D<i32>,
    },
    /// A surface displayed inside a native widget (window or view). The size of a widget surface
    /// is automatically determined based on the size of the widget. (For example, if the widget is
    /// a window, the size of the surface will be the physical size of the window.) Widget surfaces
    /// cannot be bound to textures.
    Widget {
        /// A native widget type specific to the backend.
        ///
        /// For example, on Windows this wraps an `HWND`.
        native_widget: NativeWidget,
    },
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
