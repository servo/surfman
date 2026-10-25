//! Bindings to EGL via Xlib.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

crate::implement_interfaces!(X11Connection, X11Device);

#[cfg(test)]
use self::connection::X11Connection as Connection;
#[cfg(test)]
use self::device::X11Device as Device;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
