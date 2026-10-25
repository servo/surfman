// surfman/src/platform/windows/wgl/mod.rs

//! A backend using the native Windows OpenGL WGL API.

pub mod adapter;
pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

crate::implement_interfaces!(WglConnection, WglDevice);

#[cfg(test)]
use self::connection::WglConnection as Connection;
#[cfg(test)]
use self::device::WglDevice as Device;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
