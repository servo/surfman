//! Represents the connection to the Core Graphics window server.
//!
//! Connection types are zero-sized on macOS, because the system APIs automatically manage the
//! global window server connection.

use super::device::CglDevice;
use crate::adapter::{Adapter, AdapterPreferences};
use crate::base::io_surface::connection::Connection as SystemConnection;
use crate::base::io_surface::device::NativeDevice;
use crate::Error;
use crate::GLApi;

pub use crate::base::io_surface::connection::NativeConnection;

/// A connection to the display server.
#[derive(Clone)]
pub struct CglConnection(pub SystemConnection);

impl CglConnection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<CglConnection, Error> {
        SystemConnection::new().map(CglConnection)
    }

    /// An alias for `Connection::new()`, present for consistency with other backends.
    #[inline]
    pub unsafe fn from_native_connection(
        native_connection: NativeConnection,
    ) -> Result<CglConnection, Error> {
        SystemConnection::from_native_connection(native_connection).map(CglConnection)
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        self.0.native_connection()
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    /// Returns an adapter on this system according to the provided preferences.
    #[inline]
    pub fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error> {
        self.0.create_adapter(preferences).map(Into::into)
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<CglDevice, Error> {
        self.0.create_device(adapter.apple()?).map(CglDevice)
    }

    /// An alias for `connection.create_device()` with the default adapter.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        native_device: NativeDevice,
    ) -> Result<CglDevice, Error> {
        self.0
            .create_device_from_native_device(native_device)
            .map(CglDevice)
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    pub fn from_display_handle(
        handle: raw_window_handle::DisplayHandle,
    ) -> Result<CglConnection, Error> {
        SystemConnection::from_display_handle(handle).map(CglConnection)
    }
}
