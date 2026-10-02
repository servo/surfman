//! Information related to hardware surfaces.

use crate::context::ContextID;
use crate::macros::enum_conversion;
use euclid::default::Size2D;
use raw_window_handle::WindowHandle;
use std::fmt::{self, Display, Formatter};

#[cfg(all(windows_platform, feature = "sm-angle"))]
use crate::angle::surface::AngleSurface;
#[cfg(macos_platform)]
use crate::cgl::surface::CglSurface;
#[cfg(any(android_platform, ohos_platform))]
use crate::hardware_buffer::surface::HardwareBufferSurface;
#[cfg(free_unix)]
use crate::mesa_surfaceless::surface::SurfacelessMesaSurface;
#[cfg(wayland_platform)]
use crate::wayland::surface::WaylandSurface;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::surface::WglSurface;
#[cfg(x11_platform)]
use crate::x11::surface::X11Surface;

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
    pub framebuffer_object: Option<glow::Framebuffer>,
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
#[derive(Clone)]
pub enum SurfaceType<'a> {
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
        /// A [`WindowHandle`] which identifies the widget to make this surface for.
        window_handle: WindowHandle<'a>,
        /// The size of the window to create this surface for.
        ///
        /// Note: This is currently only used for Wayland.
        size: Size2D<i32>,
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

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
///
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
///
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
#[derive(Debug)]
pub enum Surface {
    /// An ANGLE surface for Windows systems.
    #[cfg(all(windows_platform, feature = "sm-angle"))]
    Angle(AngleSurface),
    /// A surface for Apple systems.
    #[cfg(macos_platform)]
    Cgl(CglSurface),
    /// A hardware buffer surface for OHOS and Android systems.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(HardwareBufferSurface),
    /// A surfaceless Mesa surface for X11 / Wayland systems.
    #[cfg(free_unix)]
    SurfacelessMesa(SurfacelessMesaSurface),
    /// A surface for Wayland systems.
    #[cfg(wayland_platform)]
    Wayland(WaylandSurface),
    /// A WGL surface for Windows systems.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglSurface),
    /// A surface for X11 systems.
    #[cfg(x11_platform)]
    X11(X11Surface),
}

#[cfg(all(windows_platform, feature = "sm-angle"))]
enum_conversion!(Surface, Angle, AngleSurface, angle, IncompatibleSurface);
#[cfg(macos_platform)]
enum_conversion!(Surface, Cgl, CglSurface, cgl, IncompatibleSurface);
#[cfg(any(android_platform, ohos_platform))]
enum_conversion!(
    Surface,
    HardwareBuffer,
    HardwareBufferSurface,
    hardware_buffer,
    IncompatibleSurface
);
#[cfg(free_unix)]
enum_conversion!(
    Surface,
    SurfacelessMesa,
    SurfacelessMesaSurface,
    surfaceless_mesa,
    IncompatibleSurface
);
#[cfg(wayland_platform)]
enum_conversion!(
    Surface,
    Wayland,
    WaylandSurface,
    wayland,
    IncompatibleSurface
);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(Surface, Wgl, WglSurface, wgl, IncompatibleSurface);
#[cfg(x11_platform)]
enum_conversion!(Surface, X11, X11Surface, x11, IncompatibleSurface);
