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
pub use platform::default::context::Context;
pub use platform::default::device::Device;
pub use platform::default::surface::{Surface, SurfaceTexture};

pub mod error;
pub use crate::error::{Error, WindowingApiError};

/*
mod framebuffer;

mod gl_context;
pub use gl_context::{GLContext, GLContextDispatcher, GLVersion};

mod gl_context_attributes;
pub use gl_context_attributes::GLContextAttributes;

mod gl_context_capabilities;
pub use gl_context_capabilities::GLContextCapabilities;

mod gl_feature;
pub use gl_feature::GLFeature;

mod gl_formats;
pub use gl_formats::{Format, GLFormats};
*/

mod gl_limits;
pub use crate::gl_limits::GLLimits;

mod gl_info;
pub use crate::gl_info::{ContextAttributes, ContextAttributeFlags, FeatureFlags, GLApi, GLFlavor};
pub use crate::gl_info::{GLInfo, GLVersion};

mod surface;
pub use crate::surface::{SurfaceDescriptor, SurfaceFormat, SurfaceId};

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
#[allow(improper_ctypes)]
mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
#[allow(improper_ctypes)]
mod glx_extra {
    include!(concat!(env!("OUT_DIR"), "/glx_extra_bindings.rs"));
}

#[cfg(any(target_os="android", all(target_os="windows", feature = "sm-no-wgl")))]
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
