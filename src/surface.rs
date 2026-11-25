//! Information related to hardware surfaces.

use crate::context::ContextID;
use crate::macros::enum_conversion;
use crate::Error;
use euclid::default::Size2D;
use std::fmt::{self, Display, Formatter};

#[cfg(macos_platform)]
use crate::base::io_surface::buffer::IoSurfaceBuffer;
#[cfg(macos_platform)]
use crate::cgl::surface::CglWindowSurface;
#[cfg(free_unix)]
use crate::free_unix::surface::PbufferSurface;
#[cfg(android_platform)]
use crate::hardware_buffer::surface::{AndroidBufferSurface, AndroidWindowSurface};
#[cfg(ohos_platform)]
use crate::hardware_buffer::surface::{OhosBufferSurface, OhosWindowSurface};
#[cfg(wayland_platform)]
use crate::wayland::surface::WaylandWindowSurface;
#[cfg(windows_platform)]
use crate::windows::surface::{Direct3dSurface, Win32Surface};
#[cfg(x11_platform)]
use crate::x11::surface::X11WindowSurface;

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
/// Surfaces come in two varieties: `WindowSurface` and `OffscreenSurface`. Generic surfaces can be
/// bound to a texture but cannot be displayed in a widget (without using other APIs such as Core
/// Animation, DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be
/// displayed in a widget but not bound to a texture.
///
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with [`crate::Device::destroy_window_surface`] or
/// [`crate::Device::destroy_offscreen_surface`], or a panic will occur.
#[derive(Debug)]
pub enum Surface {
    /// A surface displayed in a native widget, created with
    /// [`crate::Device::create_window_surface`].
    Window(WindowSurface),
    /// An off-screen surface, created with [`crate::Device::create_offscreen_surface`].
    Offscreen(OffscreenSurface),
}

/// A surface displayed in a native widget (window or view).
///
/// Only window surfaces can be presented.
#[derive(Debug)]
pub enum WindowSurface {
    /// A surface displayed in an Android native window.
    #[cfg(android_platform)]
    Android(AndroidWindowSurface),
    /// A surface displayed in an AppKit view on macOS.
    #[cfg(macos_platform)]
    AppKit(CglWindowSurface),
    /// A surface displayed in an OpenHarmony native window.
    #[cfg(ohos_platform)]
    Ohos(OhosWindowSurface),
    /// A surface displayed in a Wayland surface.
    #[cfg(wayland_platform)]
    Wayland(WaylandWindowSurface),
    /// A surface displayed in a Win32 window, used by ANGLE and WGL.
    #[cfg(windows_platform)]
    Win32(Win32Surface),
    /// A surface displayed in an X11 window.
    #[cfg(x11_platform)]
    X11(X11WindowSurface),
}

/// An off-screen surface.
///
/// Only offscreen surfaces can be wrapped in a [`crate::SurfaceTexture`] for reading from other
/// contexts.
#[derive(Debug)]
pub enum OffscreenSurface {
    /// A surface backed by an Android `AHardwareBuffer`.
    #[cfg(android_platform)]
    Android(AndroidBufferSurface),
    /// A surface backed by a Direct3D 11 texture, used by ANGLE and WGL.
    #[cfg(windows_platform)]
    Direct3d(Direct3dSurface),
    /// A surface backed by an `IOSurface` on macOS.
    #[cfg(macos_platform)]
    IoSurface(IoSurfaceBuffer),
    /// A surface backed by an OpenHarmony `OH_NativeBuffer`.
    #[cfg(ohos_platform)]
    Ohos(OhosBufferSurface),
    /// A surface backed by an EGL image, used by X11, Wayland and surfaceless Mesa.
    #[cfg(free_unix)]
    Pbuffer(PbufferSurface),
}

#[cfg(android_platform)]
enum_conversion!(
    WindowSurface,
    Android,
    AndroidWindowSurface,
    android,
    IncompatibleSurface
);
#[cfg(macos_platform)]
enum_conversion!(
    WindowSurface,
    AppKit,
    CglWindowSurface,
    app_kit,
    IncompatibleSurface
);
#[cfg(ohos_platform)]
enum_conversion!(
    WindowSurface,
    Ohos,
    OhosWindowSurface,
    ohos,
    IncompatibleSurface
);
#[cfg(wayland_platform)]
enum_conversion!(
    WindowSurface,
    Wayland,
    WaylandWindowSurface,
    wayland,
    IncompatibleSurface
);
#[cfg(windows_platform)]
enum_conversion!(
    WindowSurface,
    Win32,
    Win32Surface,
    win32,
    IncompatibleSurface
);
#[cfg(x11_platform)]
enum_conversion!(
    WindowSurface,
    X11,
    X11WindowSurface,
    x11,
    IncompatibleSurface
);

#[cfg(android_platform)]
enum_conversion!(
    OffscreenSurface,
    Android,
    AndroidBufferSurface,
    android,
    IncompatibleSurface
);
#[cfg(windows_platform)]
enum_conversion!(
    OffscreenSurface,
    Direct3d,
    Direct3dSurface,
    direct3d,
    IncompatibleSurface
);
#[cfg(macos_platform)]
enum_conversion!(
    OffscreenSurface,
    IoSurface,
    IoSurfaceBuffer,
    io_surface,
    IncompatibleSurface
);
#[cfg(ohos_platform)]
enum_conversion!(
    OffscreenSurface,
    Ohos,
    OhosBufferSurface,
    ohos,
    IncompatibleSurface
);
#[cfg(free_unix)]
enum_conversion!(
    OffscreenSurface,
    Pbuffer,
    PbufferSurface,
    pbuffer,
    IncompatibleSurface
);

impl Surface {
    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()` on
    /// the framebuffer object returned by this function. This framebuffer object may or may not be
    /// 0, the default framebuffer, depending on platform.
    pub fn info(&self) -> SurfaceInfo {
        match self {
            Surface::Window(surface) => surface.info(),
            Surface::Offscreen(surface) => surface.info(),
        }
    }
}

impl WindowSurface {
    /// Returns various information about the surface. See [`Surface::info`].
    pub fn info(&self) -> SurfaceInfo {
        match self {
            #[cfg(android_platform)]
            WindowSurface::Android(surface) => surface.0.info(),
            #[cfg(macos_platform)]
            WindowSurface::AppKit(surface) => surface.0.info(),
            #[cfg(ohos_platform)]
            WindowSurface::Ohos(surface) => surface.0.info(),
            #[cfg(wayland_platform)]
            WindowSurface::Wayland(surface) => surface.0.info(),
            #[cfg(windows_platform)]
            WindowSurface::Win32(surface) => surface.info(),
            #[cfg(x11_platform)]
            WindowSurface::X11(surface) => surface.0.info(),
        }
    }
}

impl OffscreenSurface {
    /// Returns various information about the surface. See [`Surface::info`].
    pub fn info(&self) -> SurfaceInfo {
        match self {
            #[cfg(android_platform)]
            OffscreenSurface::Android(surface) => surface.0.info(),
            #[cfg(windows_platform)]
            OffscreenSurface::Direct3d(surface) => surface.info(),
            #[cfg(macos_platform)]
            OffscreenSurface::IoSurface(surface) => surface.info(),
            #[cfg(ohos_platform)]
            OffscreenSurface::Ohos(surface) => surface.0.info(),
            #[cfg(free_unix)]
            OffscreenSurface::Pbuffer(surface) => surface.0.info(),
        }
    }
}

impl From<WindowSurface> for Surface {
    fn from(surface: WindowSurface) -> Surface {
        Surface::Window(surface)
    }
}

impl From<OffscreenSurface> for Surface {
    fn from(surface: OffscreenSurface) -> Surface {
        Surface::Offscreen(surface)
    }
}

impl TryFrom<Surface> for WindowSurface {
    type Error = (Error, Surface);
    fn try_from(surface: Surface) -> Result<WindowSurface, Self::Error> {
        match surface {
            Surface::Window(surface) => Ok(surface),
            Surface::Offscreen(_) => Err((Error::NoWidgetAttached, surface)),
        }
    }
}

impl TryFrom<Surface> for OffscreenSurface {
    type Error = (Error, Surface);
    fn try_from(surface: Surface) -> Result<OffscreenSurface, Self::Error> {
        match surface {
            Surface::Offscreen(surface) => Ok(surface),
            Surface::Window(_) => Err((Error::WidgetAttached, surface)),
        }
    }
}
