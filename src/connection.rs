//! The abstract interface that all connections conform to.

use crate::adapter::AdapterPreferences;
use crate::Adapter;
use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;

use std::os::raw::c_void;

/// Methods relating to display server connections.
pub trait Connection: Sized {
    /// The device type associated with this connection.
    type Device;
    /// The native widget type associated with this connection.
    type NativeWidget;

    /// Connects to the default display.
    fn new() -> Result<Self, Error>;

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    fn gl_api(&self) -> GLApi;

    /// Returns an adapter on this system according to the provided preferences.
    fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error>;

    /// Opens a device.
    fn create_device(&self, adapter: &Adapter) -> Result<Self::Device, Error>;

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    fn from_display_handle(handle: raw_window_handle::DisplayHandle) -> Result<Self, Error>;

    /// Creates a native widget from a raw pointer
    unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        size: Size2D<i32>,
    ) -> Self::NativeWidget;

    /// Create a native widget type from the given `WindowHandle`.
    fn create_native_widget_from_window_handle(
        &self,
        window: raw_window_handle::WindowHandle,
        size: Size2D<i32>,
    ) -> Result<Self::NativeWidget, Error>;
}
