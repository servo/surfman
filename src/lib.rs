extern crate gleam;
extern crate libc;
extern crate euclid;

#[cfg(target_os="linux")]
extern crate x11;
#[cfg(target_os="macos")]
extern crate cgl;
#[cfg(target_os="macos")]
extern crate core_foundation;

mod platform;
pub use platform::{NativeGLContext, NativeGLContextMethods};

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
mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

#[cfg(target_os="android")]
#[allow(non_camel_case_types)]
mod egl {
    use libc;
    pub type khronos_utime_nanoseconds_t = khronos_uint64_t;
    pub type khronos_uint64_t = libc::uint64_t;
    pub type khronos_ssize_t = libc::c_long;
    pub type EGLint = libc::int32_t;
    pub type EGLNativeDisplayType = *const libc::c_void;
    pub type EGLNativePixmapType = *const libc::c_void;
    pub type EGLNativeWindowType = *const libc::c_void;
    pub type NativeDisplayType = EGLNativeDisplayType;
    pub type NativePixmapType = EGLNativePixmapType;
    pub type NativeWindowType = EGLNativeWindowType;
    include!(concat!(env!("OUT_DIR"), "/egl_bindings.rs"));
}

#[cfg(feature="texture_surface")]
extern crate layers;
#[cfg(feature="texture_surface")]
mod layers_surface_wrapper;
#[cfg(feature="texture_surface")]
pub use layers_surface_wrapper::LayersSurfaceWrapper;

#[cfg(test)]
mod tests;
