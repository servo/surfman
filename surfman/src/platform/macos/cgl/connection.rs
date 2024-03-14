// surfman/surfman/src/platform/macos/cgl/connection.rs
//
//! Represents the connection to the Core Graphics window server.
//!
//! Connection types are zero-sized on macOS, because the system APIs automatically manage the
//! global window server connection.

use super::device::{Adapter, Device};
use crate::platform::macos::system::connection::Connection as SystemConnection;
use crate::platform::macos::system::device::NativeDevice;
use crate::platform::macos::system::surface::NativeWidget;
use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;

use std::os::raw::c_void;

pub use crate::platform::macos::system::connection::NativeConnection;

/// A connection to the display server.
#[derive(Clone)]
pub struct Connection(pub SystemConnection);

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        SystemConnection::new().map(Connection)
    }

    /// An alias for `Connection::new()`, present for consistency with other backends.
    #[inline]
    pub unsafe fn from_native_connection(
        native_connection: NativeConnection,
    ) -> Result<Connection, Error> {
        SystemConnection::from_native_connection(native_connection).map(Connection)
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

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    ///
    /// This is an alias for `Connection::create_hardware_adapter()`.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_adapter().map(Adapter)
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_hardware_adapter().map(Adapter)
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        self.0.create_low_power_adapter().map(Adapter)
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
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

    /// An alias for `connection.create_device()` with the default adapter.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        native_device: NativeDevice,
    ) -> Result<Device, Error> {
        self.0
            .create_device_from_native_device(native_device)
            .map(Device)
    }

    /// Opens the display connection corresponding to the given `RawDisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn from_raw_display_handle(
        raw_handle: rwh_05::RawDisplayHandle,
    ) -> Result<Connection, Error> {
        SystemConnection::from_raw_display_handle(raw_handle).map(Connection)
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn from_display_handle(
        handle: rwh_06::DisplayHandle,
    ) -> Result<Connection, Error> {
        SystemConnection::from_display_handle(handle).map(Connection)
    }

    /// Creates a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        size: Size2D<i32>,
    ) -> NativeWidget {
        self.0.create_native_widget_from_ptr(raw, size)
    }

    /// Create a native widget type from the given `RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    #[inline]
    pub fn create_native_widget_from_raw_window_handle(
        &self,
        raw_handle: rwh_05::RawWindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use crate::platform::macos::system::surface::NSView;
        use cocoa::base::id;
        use rwh_05::RawWindowHandle::AppKit;

        match raw_handle {
            AppKit(handle) => Ok(NativeWidget {
                view: NSView(unsafe { msg_send![handle.ns_view as id, retain] }),
                opaque: unsafe { msg_send![handle.ns_window as id, isOpaque] },
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }

    /// Create a native widget type from the given `WindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    #[inline]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: rwh_06::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use crate::platform::macos::system::surface::NSView;
        use cocoa::base::id;
        use rwh_06::RawWindowHandle::AppKit;

        match handle.as_raw() {
            AppKit(handle) => Ok(NativeWidget {
                view: NSView(unsafe { msg_send![handle.ns_view as id, retain] }),
                opaque: unsafe { msg_send![handle.ns_window as id, isOpaque] },
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }
}
