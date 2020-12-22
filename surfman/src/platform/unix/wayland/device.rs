// surfman/surfman/src/platform/unix/wayland/device.rs
//
//! A wrapper around Wayland `EGLDisplay`s.

use super::connection::{Connection, NativeConnectionWrapper};
use crate::{Error, GLApi};

use std::sync::Arc;

pub use crate::platform::unix::generic::device::Adapter;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
    pub(crate) adapter: Adapter,
}

/// Wraps an adapter.
///
/// On Wayland, devices and adapters are essentially identical types.
#[derive(Clone)]
pub struct NativeDevice {
    /// The hardware adapter corresponding to this device.
    pub adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device {
            native_connection: connection.native_connection.clone(),
            adapter: (*adapter).clone(),
        })
    }

    /// Returns the native device corresponding to this device.
    ///
    /// This method is essentially an alias for the `adapter()` method on Wayland, since there is
    /// no explicit concept of a device on this backend.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice {
            adapter: self.adapter(),
        }
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection {
            native_connection: self.native_connection.clone(),
        }
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
