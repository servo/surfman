// surfman/surfman/src/platform/unix/x11/mod.rs
//
//! Bindings to EGL via Xlib.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

#[path = "../../../implementation/mod.rs"]
mod implementation;

#[cfg(test)]
#[path = "../../../tests.rs"]
mod tests;
