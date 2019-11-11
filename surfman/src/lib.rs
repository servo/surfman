// surfman/surfman/src/lib.rs
//
//! Cross-platform GPU device and surface management.
//!
//! You can use this crate to multithread a graphics application so that rendering happens on
//! multiple threads, sharing textures among them in the most efficient manner. It may also be
//! useful as a lightweight framework for *just* initializing rendering in native applications.
//! This is in contrast to crates like SDL, GLFW, winit, and Glutin, all of which have a broader
//! focus in that they manage windowing and the event loop as well.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod platform;
pub use platform::default::adapter::Adapter;
pub use platform::default::connection::Connection;
pub use platform::default::context::{Context, ContextDescriptor};
pub use platform::default::device::Device;
pub use platform::default::surface::{NativeWidget, Surface, SurfaceTexture};

// TODO(pcwalton): Fill this in with other OS's.
#[cfg(target_os = "macos")]
pub use platform::system::adapter::Adapter as SystemAdapter;
#[cfg(target_os = "macos")]
pub use platform::system::connection::Connection as SystemConnection;
#[cfg(target_os = "macos")]
pub use platform::system::device::Device as SystemDevice;
#[cfg(target_os = "macos")]
pub use platform::system::surface::Surface as SystemSurface;

pub mod connection;
pub mod device;

pub mod error;
pub use crate::error::{Error, WindowingApiError};

mod context;
pub use crate::context::{ContextAttributes, ContextAttributeFlags, ContextID};

mod info;
pub use crate::info::{GLApi, GLVersion};

mod surface;
pub use crate::surface::{SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType, SystemSurfaceInfo};

pub mod macros;

mod gl_utils;
mod renderbuffers;

#[cfg(test)]
mod tests;

mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[cfg(any(feature = "sm-x11", all(unix, not(any(target_os = "macos", target_os = "android")))))]
mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

#[cfg(any(target_os = "android", all(target_os = "windows", feature = "sm-angle"), unix))]
#[allow(non_camel_case_types)]
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
