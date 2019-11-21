// surfman/surfman/src/platform/unix/x11/device.rs
//
//! Thread-local handles to devices.

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

/// A zero-sized type that represents a native device in X11.
///
/// GLX has no explicit concept of a hardware device, so this type only contains an adapter.
#[derive(Clone)]
pub struct NativeDevice {
    /// The hardware adapter corresponding to this device.
    pub adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device { connection: (*connection).clone(), adapter: (*adapter).clone() })
    }

    /// Returns the native device corresponding to this device.
    ///
    /// This method is essentially an alias for the `adapter()` method on X11, since there is no
    /// explicit concept of a device on this backend.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice { adapter: self.adapter() }
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
}

