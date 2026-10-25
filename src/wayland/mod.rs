//! Bindings to Wayland via the Linux GBM interface.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

crate::implement_interfaces!(WaylandConnection, WaylandDevice);

#[cfg(test)]
use self::connection::WaylandConnection as Connection;
#[cfg(test)]
use self::device::WaylandDevice as Device;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
