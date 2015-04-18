#![feature(unsafe_destructor)]

extern crate gleam;
extern crate libc;
extern crate geom;

#[cfg(target_os="linux")]
extern crate xlib;
#[cfg(target_os="linux")]
extern crate glx;

pub mod platform;
pub use platform::GLContext;

pub mod common_methods;
pub use common_methods::GLContextMethods;

pub mod draw_buffer;
pub use draw_buffer::DrawBuffer;

pub mod gl_context_attributes;
pub use gl_context_attributes::GLContextAttributes;

pub mod gl_context_capabilities;
pub use gl_context_capabilities::GLContextCapabilities;

pub mod gl_feature;
pub use gl_feature::GLFeature;

#[cfg(test)]
mod tests;
