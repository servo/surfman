#![feature(unsafe_destructor)]

extern crate xlib;
extern crate glx;
extern crate gleam;
extern crate libc;
extern crate geom;

pub mod platform;
pub use platform::*;

mod common_methods;
pub use common_methods::GLContextMethods;

#[cfg(test)]
mod tests;
