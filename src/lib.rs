#![feature(unsafe_destructor)]

extern crate gleam;
extern crate libc;
extern crate geom;

#[cfg(target_os="linux")]
extern crate xlib;
#[cfg(target_os="linux")]
extern crate glx;
#[cfg(target_os="macos")]
extern crate cgl;

mod platform;
pub use platform::{NativeGLContext, NativeGLContextMethods};

mod gl_context;
pub use gl_context::GLContext;

mod draw_buffer;
pub use draw_buffer::DrawBuffer;

mod gl_context_attributes;
pub use gl_context_attributes::GLContextAttributes;

mod gl_context_capabilities;
pub use gl_context_capabilities::GLContextCapabilities;

mod gl_feature;
pub use gl_feature::GLFeature;

#[cfg(test)]
mod tests;
