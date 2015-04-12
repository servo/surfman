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

#[cfg(test)]
mod tests;
