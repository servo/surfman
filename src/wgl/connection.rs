//! A connection to the window server.
//!
//! Window server connections are implicit in the Win32 API, so this is a zero-sized type.

use super::adapter::WglAdapter;
use super::device::{Device, NativeDevice};
use super::surface::NativeWidget;
use crate::{Adapter, AdapterPreferences, Error, GLApi};

use euclid::default::Size2D;

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

    /// Returns an adapter on this system according to the provided preferences.
    #[inline]
    pub fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error> {
        Ok(WglAdapter::new(preferences).into())
    }

    /// Opens a device.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(adapter.wgl()?)
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

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    pub fn from_display_handle(_: raw_window_handle::DisplayHandle) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Create a native widget type from the given `WindowHandle`.
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: raw_window_handle::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use raw_window_handle::RawWindowHandle::Win32;

        match handle.as_raw() {
            Win32(handle) => Ok(NativeWidget {
                window_handle: handle.hwnd.get() as HWND,
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }
}

impl Default for NativeConnection {
    fn default() -> Self {
        Self::new()
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
