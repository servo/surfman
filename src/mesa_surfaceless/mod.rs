//! The Mesa "surfaceless" backend, which only supports off-screen surfaces and cannot directly
//! display surfaces on a screen.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

crate::implement_interfaces!(SurfacelessMesaConnection, SurfacelessMesaDevice);

#[cfg(test)]
use self::connection::SurfacelessMesaConnection as Connection;
#[cfg(test)]
use self::device::SurfacelessMesaDevice as Device;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
