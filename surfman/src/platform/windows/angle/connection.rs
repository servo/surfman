// surfman/surfman/src/platform/windows/angle/connection.rs
//
//! A connection to the window server.
//! 
//! It might seem like this should wrap an `EGLDisplay`, but it doesn't. Unfortunately, in the
//! ANGLE implementation `EGLDisplay` is not thread-safe, while `surfman` connections must be
//! thread-safe. So we need to use the DXGI/Direct3D concept of a connection instead. These are
//! implicit in the Win32 API, and as such this type is a no-op.

use crate::Error;
use super::device::{Adapter, Device};
use super::surface::NativeWidget;

use winapi::shared::windef::HWND;
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_HARDWARE;
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_UNKNOWN;
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_WARP;

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
        Adapter::from_driver_type(D3D_DRIVER_TYPE_UNKNOWN)
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Adapter::from_driver_type(D3D_DRIVER_TYPE_HARDWARE)
    }

    /// Returns the "best" low-power hardware adapter on this system.
    ///
    /// TODO(pcwalton)
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Adapter::from_driver_type(D3D_DRIVER_TYPE_HARDWARE)
    }

    /// Returns the "best" software adapter on this system.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Adapter::from_driver_type(D3D_DRIVER_TYPE_WARP)
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
