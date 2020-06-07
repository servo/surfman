// surfman/surfman/src/platform/windows/angle/mod.rs
//
//! Bindings to Direct3D 11 via the ANGLE OpenGL-to-Direct3D translation layer on Windows.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

#[path = "../../../implementation/mod.rs"]
mod implementation;

#[cfg(test)]
#[path = "../../../tests.rs"]
mod tests;
