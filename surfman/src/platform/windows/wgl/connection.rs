// surfman/surfman/src/platform/windows/wgl/connection.rs
//
//! A connection to the window server.
//!
//! Window server connections are implicit in the Win32 API, so this is a zero-sized type.

use super::device::{Adapter, Device, NativeDevice};
use super::surface::NativeWidget;
use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;

use std::os::raw::c_void;

use winapi::shared::windef::HWND;

/// Represents a connection to the display server.
///
/// Window server connections are implicit in the Win32 API, so this is a zero-sized type.
#[derive(Clone)]
pub struct Connection;

/// An empty placeholder for native connections.
///
/// Window server connections are implicit in the Win32 API, so this is a zero-sized type.
#[derive(Clone)]
pub struct NativeConnection;

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection)
    }

    /// An alias for `Connection::new()`, present for consistency with other backends.
    #[inline]
    pub unsafe fn from_native_connection(_: NativeConnection) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    ///
    /// This is an alias for `Connection::create_hardware_adapter()`.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::HighPerformance)
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::LowPower)
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        self.create_low_power_adapter()
    }

    /// Opens a device.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(adapter)
    }

    /// Creates a `Device` from a Direct3D 11 device and associated GL/DX interop handle.
    ///
    /// The handle can be created by calling `wglDXOpenDeviceNV` from the `WGL_NV_DX_interop`
    /// extension.
    ///
    /// This method increases the reference count on the Direct3D 11 device and takes ownership of
    /// the GL/DX interop handle.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        native_device: NativeDevice,
    ) -> Result<Device, Error> {
        Device::from_native_device(native_device)
    }

    /// Opens the display connection corresponding to the given `RawDisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn from_raw_display_handle(
        _: rwh_05::RawDisplayHandle,
    ) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn from_display_handle(
        _: rwh_06::DisplayHandle,
    ) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        _size: Size2D<i32>,
    ) -> NativeWidget {
        NativeWidget {
            window_handle: raw as HWND,
        }
    }

    /// Create a native widget type from the given `RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn create_native_widget_from_raw_window_handle(
        &self,
        raw_handle: rwh_05::RawWindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use rwh_05::RawWindowHandle::Win32;

        match raw_handle {
            Win32(handle) => Ok(NativeWidget {
                window_handle: handle.hwnd as HWND,
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }

    /// Create a native widget type from the given `WindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: rwh_06::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use rwh_06::RawWindowHandle::Win32;

        match handle.as_raw() {
            Win32(handle) => Ok(NativeWidget {
                window_handle: handle.hwnd.get() as HWND,
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }
}

impl NativeConnection {
    /// Creates a native connection.
    ///
    /// This is a no-op method present for consistency with other backends.
    #[inline]
    pub fn new() -> NativeConnection {
        NativeConnection
    }
}
