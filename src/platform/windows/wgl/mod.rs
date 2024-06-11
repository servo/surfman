// surfman/src/platform/windows/wgl/mod.rs

//! A backend using the native Windows OpenGL WGL API.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

#[path = "../../../implementation/mod.rs"]
mod implementation;

#[cfg(test)]
#[path = "../../../tests.rs"]
mod tests;
