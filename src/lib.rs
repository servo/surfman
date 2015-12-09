#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]

extern crate gleam;
extern crate euclid;
extern crate serde;

#[cfg(target_os="linux")]
extern crate x11;
#[cfg(target_os="macos")]
extern crate cgl;
#[cfg(target_os="macos")]
extern crate core_foundation;

mod platform;
pub use platform::{NativeGLContext, NativeGLContextMethods, NativeGLContextHandle};

mod gl_context;
pub use gl_context::GLContext;

mod draw_buffer;
pub use draw_buffer::{DrawBuffer, ColorAttachmentType};

mod gl_context_attributes;
pub use gl_context_attributes::GLContextAttributes;

mod gl_context_capabilities;
pub use gl_context_capabilities::GLContextCapabilities;

mod gl_feature;
pub use gl_feature::GLFeature;

mod gl_formats;
pub use gl_formats::GLFormats;

#[macro_use]
extern crate log;

#[cfg(target_os="linux")]
#[allow(improper_ctypes)]
mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

#[cfg(any(target_os="linux", target_os="android"))]
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
