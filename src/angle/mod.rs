//! Bindings to Direct3D 11 via the ANGLE OpenGL-to-Direct3D translation layer on Windows.

pub mod adapter;
pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

crate::implement_interfaces!(AngleConnection);

#[cfg(test)]
use self::connection::AngleConnection as Connection;
#[cfg(test)]
use self::device::AngleDevice as ConcreteDevice;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
