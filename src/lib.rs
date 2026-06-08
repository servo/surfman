//! Cross-platform GPU device and surface management.
//!
//! You can use this crate to multithread a graphics application so that rendering happens on
//! multiple threads, sharing textures among them in the most efficient manner. It may also be
//! useful as a lightweight framework for *just* initializing rendering in native applications.
//! This is in contrast to crates like SDL, GLFW, winit, and Glutin, all of which have a broader
//! focus in that they manage windowing and the event loop as well.

#![warn(missing_docs)]

#[macro_use]
extern crate bitflags;
#[allow(unused_imports)]
#[macro_use]
extern crate log;

#[cfg(all(windows_platform, feature = "sm-angle"))]
pub mod angle;
pub(crate) mod base;
#[cfg(macos_platform)]
pub mod cgl;
#[cfg(feature = "chains")]
pub mod chains;
pub mod connection;
mod context;
pub mod device;
pub mod error;
mod gl_utils;
#[cfg(any(android_platform, ohos_platform))]
pub mod hardware_buffer;
mod info;
pub mod macros;
#[cfg(free_unix)]
pub mod mesa_surfaceless;
pub mod multi;
mod renderbuffers;
mod surface;
#[cfg(all(x11_platform, not(wayland_default)))]
pub mod unix;
#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
pub mod wgl;
#[cfg(x11_platform)]
pub mod x11;

#[cfg(all(windows_platform, angle_default))]
pub use angle as default;
#[cfg(macos_platform)]
pub use cgl as default;
#[cfg(any(android_platform, ohos_platform))]
pub use hardware_buffer as default;
#[cfg(all(x11_platform, not(wayland_default)))]
pub use unix as default;
#[cfg(wayland_default)]
pub use wayland as default;
#[cfg(all(windows_platform, not(angle_default)))]
pub use wgl as default;

pub use crate::context::{ContextAttributeFlags, ContextAttributes, ContextID};
pub use crate::error::{Error, WindowingApiError};
pub use crate::info::{GLApi, GLVersion};
pub use crate::surface::{SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType, SystemSurfaceInfo};
pub use default::connection::{Connection, NativeConnection};
pub use default::context::{Context, ContextDescriptor, NativeContext};
pub use default::device::{Adapter, Device, NativeDevice};
pub use default::surface::{NativeWidget, Surface, SurfaceTexture};
pub(crate) use glow::{self as gl, Context as Gl};
pub(crate) use macros::implement_interfaces;

// TODO(pcwalton): Fill this in with other OS's.
#[cfg(target_os = "macos")]
pub use base::io_surface::connection::Connection as SystemConnection;
#[cfg(target_os = "macos")]
pub use base::io_surface::device::{Adapter as SystemAdapter, Device as SystemDevice};
#[cfg(target_os = "macos")]
pub use base::io_surface::surface::Surface as SystemSurface;

#[cfg(any(
    target_os = "android",
    target_env = "ohos",
    all(target_os = "windows", feature = "sm-angle"),
    unix
))]
#[allow(non_camel_case_types)]
#[allow(clippy::all)]
mod egl {
    use std::os::raw::{c_long, c_void};
    pub type khronos_utime_nanoseconds_t = khronos_uint64_t;
    pub type khronos_uint64_t = u64;
    pub type khronos_ssize_t = c_long;
    pub type EGLint = i32;
    pub type EGLNativeDisplayType = *const c_void;
    pub type EGLNativePixmapType = *const c_void;
    pub type EGLNativeWindowType = *const c_void;
    pub type NativeDisplayType = EGLNativeDisplayType;
    pub type NativePixmapType = EGLNativePixmapType;
    pub type NativeWindowType = EGLNativeWindowType;
    include!(concat!(env!("OUT_DIR"), "/egl_bindings.rs"));
}
