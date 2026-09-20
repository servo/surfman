//! A no-op connection for Android.
//!
//! FIXME(pcwalton): Should this instead wrap `EGLDisplay`? Is that thread-safe on Android?

use super::adapter::HardwareBufferAdapter;
use super::device::{Device, NativeDevice};
use super::surface::NativeWidget;
use crate::{Adapter, AdapterPreferences, Error, GLApi};

use euclid::default::Size2D;

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

    /// Returns an adapter on this system according to the provided preferences.
    #[inline]
    pub fn create_adapter(&self, _: AdapterPreferences) -> Result<Adapter, Error> {
        Ok(HardwareBufferAdapter.into())
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

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    pub fn from_display_handle(_: raw_window_handle::DisplayHandle) -> Result<Connection, Error> {
        Ok(Connection)
    }

    #[cfg(android_platform)]
    #[inline]
    fn create_native_widget_from_raw_window_handle(
        handle: raw_window_handle::WindowHandle,
    ) -> Result<NativeWidget, Error> {
        use raw_window_handle::RawWindowHandle::AndroidNdk;

        match handle.as_raw() {
            AndroidNdk(handle) => Ok(NativeWidget {
                native_window: handle.a_native_window.as_ptr() as *mut _,
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }

    #[cfg(ohos_platform)]
    #[inline]
    fn create_native_widget_from_raw_window_handle(
        handle: raw_window_handle::WindowHandle,
    ) -> Result<NativeWidget, Error> {
        use raw_window_handle::RawWindowHandle::OhosNdk;

        match handle.as_raw() {
            OhosNdk(handle) => Ok(NativeWidget {
                native_window: handle.native_window.as_ptr().cast(),
            }),
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }

    /// Create a native widget type from the given `WindowHandle`.
    #[inline]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: raw_window_handle::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        Self::create_native_widget_from_raw_window_handle(handle)
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
