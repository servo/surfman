//! Declarations common to all platform contexts.

#![allow(unused_imports)]

use crate::gl;
use crate::info::GLVersion;
use crate::macros::enum_conversion;
use crate::Gl;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

#[cfg(all(windows_platform, feature = "sm-angle"))]
use crate::angle::context::AngleContext;
#[cfg(macos_platform)]
use crate::cgl::context::CglContext;
#[cfg(any(android_platform, ohos_platform))]
use crate::hardware_buffer::context::HardwareBufferContext;
#[cfg(free_unix)]
use crate::mesa_surfaceless::context::SurfacelessMesaContext;
#[cfg(wayland_platform)]
use crate::wayland::context::WaylandContext;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::context::WglContext;
#[cfg(x11_platform)]
use crate::x11::context::X11Context;

/// A unique ID among all currently-allocated contexts.
///
/// If you destroy a context, subsequently-allocated contexts might reuse the same ID.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContextID(pub u64);

#[doc(hidden)]
pub static CREATE_CONTEXT_MUTEX: Mutex<ContextID> = Mutex::new(ContextID(0));

bitflags! {
    /// Various flags that control attributes of the context and/or surfaces created from that
    /// context.
    ///
    /// These roughly correspond to:
    /// https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
    ///
    /// There are some extra `surfman`-specific flags as well.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct ContextAttributeFlags: u8 {
        /// Surfaces created for this context will have an alpha channel (RGBA or BGRA; i.e. 4
        /// channels, 32 bits per pixel, 8 bits per channel). If this is not present, surfaces will
        /// be RGBX or BGRX (i.e. 3 channels, 32 bits per pixel, 8 bits per channel).
        const ALPHA                 = 0x01;
        /// Surfaces created for this context will have a 24-bit depth buffer.
        const DEPTH                 = 0x02;
        /// Surfaces created for this context will have an 8-bit stencil buffer, possibly using
        /// packed depth/stencil if the GL implementation supports it.
        const STENCIL               = 0x04;
        /// The OpenGL compatibility profile will be used. If this is not present, the core profile
        /// is used.
        const COMPATIBILITY_PROFILE = 0x08;
    }
}

/// Attributes that control aspects of a context and/or surfaces created from that context.
///
/// Similar to: <https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES>
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ContextAttributes {
    /// The OpenGL or OpenGL ES version that this context supports.
    ///
    /// Keep in mind that OpenGL and OpenGL ES have different version numbering schemes. Before
    /// filling in this field, check the result of `Device::gl_api()`.
    pub version: GLVersion,
    /// Various flags.
    pub flags: ContextAttributeFlags,
}

impl ContextAttributes {
    #[allow(dead_code)]
    pub(crate) fn zeroed() -> ContextAttributes {
        ContextAttributes {
            version: GLVersion::new(0, 0),
            flags: ContextAttributeFlags::empty(),
        }
    }
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(crate) fn current_context_uses_compatibility_profile(_gl: &Gl) -> bool {
    false
}

#[cfg(not(any(target_os = "android", target_env = "ohos")))]
#[allow(dead_code)]
pub(crate) fn current_context_uses_compatibility_profile(gl: &Gl) -> bool {
    use glow::HasContext;

    unsafe {
        // First, try `GL_CONTEXT_PROFILE_MASK`.
        let context_profile_mask = gl.get_parameter_i32(gl::CONTEXT_PROFILE_MASK);
        if gl.get_error() == gl::NO_ERROR
            && (context_profile_mask & gl::CONTEXT_COMPATIBILITY_PROFILE_BIT as i32) != 0
        {
            return true;
        }

        // Second, look for the `GL_ARB_compatibility` extension.
        gl.supported_extensions().contains("GL_ARB_compatibility")
    }
}

/// Represents an OpenGL rendering context.
///
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
///
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
///
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
///
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub enum Context {
    /// An ANGLE context for Windows systems.
    #[cfg(all(windows_platform, feature = "sm-angle"))]
    Angle(AngleContext),
    /// A context for Apple systems.
    #[cfg(macos_platform)]
    Cgl(CglContext),
    /// A hardware buffer context for OHOS and Android systems.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(HardwareBufferContext),
    /// A surfaceless Mesa context for X11 / Wayland systems.
    #[cfg(free_unix)]
    SurfacelessMesa(SurfacelessMesaContext),
    /// A context for Wayland systems.
    #[cfg(wayland_platform)]
    Wayland(WaylandContext),
    /// A WGL context for Windows systems.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglContext),
    /// A context for X11 systems.
    #[cfg(x11_platform)]
    X11(X11Context),
}

#[cfg(all(windows_platform, feature = "sm-angle"))]
enum_conversion!(Context, Angle, AngleContext, angle, IncompatibleContext);
#[cfg(macos_platform)]
enum_conversion!(Context, Cgl, CglContext, cgl, IncompatibleContext);
#[cfg(any(android_platform, ohos_platform))]
enum_conversion!(
    Context,
    HardwareBuffer,
    HardwareBufferContext,
    hardware_buffer,
    IncompatibleContext
);
#[cfg(free_unix)]
enum_conversion!(
    Context,
    SurfacelessMesa,
    SurfacelessMesaContext,
    surfaceless_mesa,
    IncompatibleContext
);
#[cfg(wayland_platform)]
enum_conversion!(
    Context,
    Wayland,
    WaylandContext,
    wayland,
    IncompatibleContext
);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(Context, Wgl, WglContext, wgl, IncompatibleContext);
#[cfg(x11_platform)]
enum_conversion!(Context, X11, X11Context, x11, IncompatibleContext);
