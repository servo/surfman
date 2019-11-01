// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).
//!
//! FIXME(pcwalton): I think this should actually wrap the `Display`.

use crate::error::Error;

use std::ffi::CString;

#[cfg(feature = "sm-winit")]
use winit::Window;

#[derive(Clone)]
pub struct Connection {
    pub(crate) display_name: Option<CString>,
}

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection { display_name: None })
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Connection::new()
    }
}
