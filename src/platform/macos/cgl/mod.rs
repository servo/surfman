// surfman/surfman/src/platform/macos/cgl/mod.rs
//
//! Bindings to Apple's OpenGL implementation on macOS.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

mod error;
mod ffi;

crate::implement_interfaces!();

#[cfg(test)]
#[path = "../../../tests.rs"]
mod tests;
