//! Bindings to Apple's OpenGL implementation on macOS.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

mod error;
mod ffi;

crate::implement_interfaces!(CglConnection, CglDevice);

#[cfg(test)]
use self::connection::CglConnection as Connection;
#[cfg(test)]
use self::device::CglDevice as Device;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
