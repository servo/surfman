// surfman/surfman/src/platform/unix/x11/device.rs
//
//! A wrapper around X11 Displays.

use crate::glx::types::Display as GlxDisplay;
use crate::{Error, GLApi};
use super::connection::Connection;

pub use crate::platform::unix::generic::device::Adapter;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) connection: Connection,
    pub(crate) adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device { connection: (*connection).clone(), adapter: (*adapter).clone() })
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        self.connection.clone()
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    pub(crate) fn glx_display(&self) -> *mut GlxDisplay {
        self.connection.native_display.display() as *mut GlxDisplay
    }
}

