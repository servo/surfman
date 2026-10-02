//! Information related to hardware surface textures.

use crate::macros::enum_conversion;

#[cfg(all(windows_platform, feature = "sm-angle"))]
use crate::angle::surface::AngleSurfaceTexture;
#[cfg(macos_platform)]
use crate::cgl::surface::CglSurfaceTexture;
#[cfg(any(android_platform, ohos_platform))]
use crate::hardware_buffer::surface::HardwareBufferSurfaceTexture;
#[cfg(free_unix)]
use crate::mesa_surfaceless::surface::SurfacelessMesaSurfaceTexture;
#[cfg(wayland_platform)]
use crate::wayland::surface::WaylandSurfaceTexture;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::surface::WglSurfaceTexture;
#[cfg(x11_platform)]
use crate::x11::surface::X11SurfaceTexture;

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
#[derive(Debug)]
pub enum SurfaceTexture {
    /// An ANGLE surface texture for Windows systems.
    #[cfg(all(windows_platform, feature = "sm-angle"))]
    Angle(AngleSurfaceTexture),
    /// A surface texture for Apple systems.
    #[cfg(macos_platform)]
    Cgl(CglSurfaceTexture),
    /// A hardware buffer surface texture for OHOS and Android systems.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(HardwareBufferSurfaceTexture),
    /// A surfaceless Mesa surface texture for X11 / Wayland systems.
    #[cfg(free_unix)]
    SurfacelessMesa(SurfacelessMesaSurfaceTexture),
    /// A surface texture for Wayland systems.
    #[cfg(wayland_platform)]
    Wayland(WaylandSurfaceTexture),
    /// A WGL surface texture for Windows systems.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglSurfaceTexture),
    /// A surface texture for X11 systems.
    #[cfg(x11_platform)]
    X11(X11SurfaceTexture),
}

#[cfg(all(windows_platform, feature = "sm-angle"))]
enum_conversion!(
    SurfaceTexture,
    Angle,
    AngleSurfaceTexture,
    angle,
    IncompatibleSurfaceTexture
);
#[cfg(macos_platform)]
enum_conversion!(
    SurfaceTexture,
    Cgl,
    CglSurfaceTexture,
    cgl,
    IncompatibleSurfaceTexture
);
#[cfg(any(android_platform, ohos_platform))]
enum_conversion!(
    SurfaceTexture,
    HardwareBuffer,
    HardwareBufferSurfaceTexture,
    hardware_buffer,
    IncompatibleSurfaceTexture
);
#[cfg(free_unix)]
enum_conversion!(
    SurfaceTexture,
    SurfacelessMesa,
    SurfacelessMesaSurfaceTexture,
    surfaceless_mesa,
    IncompatibleSurfaceTexture
);
#[cfg(wayland_platform)]
enum_conversion!(
    SurfaceTexture,
    Wayland,
    WaylandSurfaceTexture,
    wayland,
    IncompatibleSurfaceTexture
);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(
    SurfaceTexture,
    Wgl,
    WglSurfaceTexture,
    wgl,
    IncompatibleSurfaceTexture
);
#[cfg(x11_platform)]
enum_conversion!(
    SurfaceTexture,
    X11,
    X11SurfaceTexture,
    x11,
    IncompatibleSurfaceTexture
);
