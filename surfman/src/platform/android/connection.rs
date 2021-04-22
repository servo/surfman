// surfman/surfman/src/platform/android/connection.rs
//
//! A no-op connection for Android.
//!
//! FIXME(pcwalton): Should this instead wrap `EGLDisplay`? Is that thread-safe on Android?

use super::device::{Adapter, Device, NativeDevice};
use super::ffi::ANativeWindow;
use super::surface::NativeWidget;
use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;

use std::os::raw::c_void;

#[cfg(feature = "sm-winit")]
use winit::window::Window;

/// A connection to the display server.
#[derive(Clone)]
pub struct Connection;

/// An empty placeholder for native connections.
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
        GLApi::GLES
    }

    /// Returns the "best" adapter on this system.
    ///
    /// This is an alias for `Connection::create_hardware_adapter()`.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, _: &Adapter) -> Result<Device, Error> {
        Device::new()
    }

    /// Wraps an Android `EGLDisplay` in a device and returns it.
    ///
    /// The underlying `EGLDisplay` is not retained, as there is no way to do this in the EGL API.
    /// Therefore, it is the caller's responsibility to keep it alive as long as this `Device`
    /// remains alive.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        native_device: NativeDevice,
    ) -> Result<Device, Error> {
        Ok(Device {
            egl_display: native_device.0,
            display_is_owned: false,
        })
    }

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Ok(Connection)
    }

    /// Creates a native widget type from the given `winit` window.
    ///
    /// This type can be later used to create surfaces that render to the window.
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn create_native_widget_from_winit_window(
        &self,
        window: &Window,
    ) -> Result<NativeWidget, Error> {
        use raw_window_handle::HasRawWindowHandle;
        use raw_window_handle::RawWindowHandle::Android;
        match window.raw_window_handle() {
            Android(handle) => Ok(NativeWidget {
                native_window: handle.a_native_window as *mut _,
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        _size: Size2D<i32>,
    ) -> NativeWidget {
        NativeWidget {
            native_window: raw as *mut ANativeWindow,
        }
    }

    /// Create a native widget type from the given `raw_window_handle::RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle")]
    #[inline]
    pub fn create_native_widget_from_rwh(
        &self,
        raw_handle: raw_window_handle::RawWindowHandle,
    ) -> Result<NativeWidget, Error> {
        use raw_window_handle::RawWindowHandle::Android;

        match raw_handle {
            Android(handle) => Ok(NativeWidget {
                native_window: handle.a_native_window as *mut _,
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
    pub fn current() -> Result<NativeConnection, Error> {
        Ok(NativeConnection)
    }
}
