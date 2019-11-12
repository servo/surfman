// surfman/surfman/src/platform/macos/cgl/connection.rs
//
//! Represents the connection to the Core Graphics window server.
//! 
//! This is a no-op, because the system APIs automatically manage the global window server
//! connection.

use crate::Error;
use crate::platform::macos::system::connection::Connection as SystemConnection;
use crate::platform::macos::system::surface::NativeWidget;
use super::device::{Adapter, Device};

#[cfg(feature = "sm-winit")]
use winit::Window;

/// A no-op connection.
#[derive(Clone)]
pub struct Connection(pub SystemConnection);

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        SystemConnection::new().map(Connection)
    }

    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_adapter().map(Adapter)
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_hardware_adapter().map(Adapter)
    }

    /// Returns the most energy-efficient hardware adapter on this system.
    /// 
    /// On multi-GPU systems, this will return the integrated GPU.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_low_power_adapter().map(Adapter)
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// On macOS this returns the low-power adapter.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_software_adapter().map(Adapter)
    }

    /// Opens the hardware device corresponding to the given adapter.
    /// 
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        self.0.create_device(&adapter.0).map(Device)
    }

    /// Retrieves the display server connection from the given `winit` window.
    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        SystemConnection::from_winit_window(window).map(Connection)
    }

    /// Retrieves the native widget handle from the given `winit` window.
    /// 
    /// This widget can be used to create surfaces.
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn create_native_widget_from_winit_window(&self, window: &Window)
                                                  -> Result<NativeWidget, Error> {
        self.0.create_native_widget_from_winit_window(window)
    }
}
