// surfman/surfman/src/platform/windows/wgl/connection.rs
//
//! A connection to the window server.
//! 
//! Window server handles are implicit in the Win32 API, so this is a no-op.

use crate::Error;
use super::device::{Adapter, Device};
use super::surface::NativeWidget;

use winapi::shared::windef::HWND;

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::windows::WindowExt;

/// A no-op connection.
#[derive(Clone)]
pub struct Connection;

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection)
    }

    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::HighPerformance)
    }

    /// Returns the "best" low-power hardware adapter on this system.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::LowPower)
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The WGL backend has no software support, so this returns an error.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }

    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(adapter)
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Connection::new()
    }

    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn create_native_widget_from_winit_window(&self, window: &Window)
                                                  -> Result<NativeWidget, Error> {
        let hwnd = window.get_hwnd() as HWND;
        if hwnd.is_null() {
            Err(Error::IncompatibleNativeWidget)
        } else {
            Ok(NativeWidget { window_handle: hwnd })
        }
    }
}
