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

#[cfg(target_os = "ios")]
#[macro_use]
extern crate objc;
#[cfg(target_os = "macos")]
extern crate io_surface;
#[cfg(target_os = "windows")]
extern crate wio;

pub mod platform;
pub use platform::default::adapter::Adapter;
pub use platform::default::context::{Context, ContextDescriptor};
pub use platform::default::device::Device;
pub use platform::default::loader::get_proc_address;
pub use platform::default::surface::{Surface, SurfaceTexture};

pub mod error;
pub use crate::error::{Error, WindowingApiError};

mod info;
pub use crate::info::{ContextAttributes, ContextAttributeFlags, FeatureFlags, GLApi};
pub use crate::info::{GLInfo, GLVersion};

mod limits;
pub use crate::limits::GLLimits;

mod surface;
pub use crate::surface::SurfaceId;

#[cfg(any(feature = "sm-x11", all(unix, not(any(target_os = "macos", target_os = "android")))))]
#[allow(improper_ctypes)]
mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

pub fn init() {
    gl::load_with(get_proc_address);
    platform::default::loader::init();
}

#[cfg(any(target_os = "android", target_os = "windows"))]
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

#[cfg(test)]
mod tests;
