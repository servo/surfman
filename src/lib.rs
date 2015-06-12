extern crate gleam;
extern crate libc;
extern crate geom;

#[cfg(target_os="linux")]
extern crate x11;
#[cfg(target_os="linux")]
extern crate glx;
#[cfg(target_os="macos")]
extern crate cgl;
#[cfg(target_os="macos")]
extern crate core_foundation;
#[cfg(target_os="android")]
extern crate egl;

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

#[cfg(feature="texture_surface")]
extern crate layers;
#[cfg(feature="texture_surface")]
mod layers_surface_wrapper;
#[cfg(feature="texture_surface")]
pub use layers_surface_wrapper::LayersSurfaceWrapper;

#[cfg(test)]
#[cfg(target_os="macos")]
extern crate core_foundation;

#[cfg(test)]
mod tests;
