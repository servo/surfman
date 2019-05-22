#[macro_use]
extern crate log;

#[cfg(any(not(target_os = "linux"), feature = "test_egl_in_linux"))]
#[macro_use]
extern crate lazy_static;

#[cfg(target_os = "ios")]
#[macro_use]
extern crate objc;
extern crate io_surface;

mod platform;
pub use platform::{NativeGLContext, NativeGLContextMethods, NativeGLContextHandle};

#[cfg(feature="osmesa")]
pub use platform::{OSMesaContext, OSMesaContextHandle};

mod gl_context;
pub use gl_context::{GLContext, GLContextDispatcher, GLVersion};

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

mod gl_limits;
pub use gl_limits::GLLimits;

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

#[cfg(any(
    target_os="android",
    all(target_os="windows", feature = "no_wgl"),
    all(target_os="linux", feature = "test_egl_in_linux")
))]
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
