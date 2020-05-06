// surfman/surfman/src/connection.rs
//
//! The abstract interface that all connections conform to.

use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;

use std::os::raw::c_void;

#[cfg(feature = "sm-winit")]
use winit::window::Window;

/// Methods relating to display server connections.
pub trait Connection: Sized {
    /// The adapter type associated with this connection.
    type Adapter;
    /// The device type associated with this connection.
    type Device;
    /// The native type associated with this connection.
    type NativeConnection;
    /// The native device type associated with this connection.
    type NativeDevice;
    /// The native widget type associated with this connection.
    type NativeWidget;

    /// Connects to the default display.
    fn new() -> Result<Self, Error>;

    /// Returns the native connection corresponding to this connection.
    fn native_connection(&self) -> Self::NativeConnection;

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    fn gl_api(&self) -> GLApi;

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    /// 
    /// This is an alias for `Connection::create_hardware_adapter()`.
    fn create_adapter(&self) -> Result<Self::Adapter, Error>;

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    fn create_hardware_adapter(&self) -> Result<Self::Adapter, Error>;

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    fn create_low_power_adapter(&self) -> Result<Self::Adapter, Error>;

    /// Returns the "best" adapter on this system, preferring software adapters.
    fn create_software_adapter(&self) -> Result<Self::Adapter, Error>;

    /// Opens a device.
    fn create_device(&self, adapter: &Self::Adapter) -> Result<Self::Device, Error>;

    /// Wraps an existing native device type in a device.
    unsafe fn create_device_from_native_device(&self, native_device: Self::NativeDevice)
                                               -> Result<Self::Device, Error>;

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Self, Error>;

    /// Creates a native widget type from the given `winit` window.
    /// 
    /// This type can be later used to create surfaces that render to the window.
    #[cfg(feature = "sm-winit")]
    fn create_native_widget_from_winit_window(&self, window: &Window)
                                              -> Result<Self::NativeWidget, Error>;

    /// Creates a native widget from a raw pointer
    unsafe fn create_native_widget_from_ptr(&self, raw: *mut c_void, size: Size2D<i32>) -> Self::NativeWidget;

    /// Create a native widget type from the given `raw_window_handle::RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle")]
    fn create_native_widget_from_rwh(&self, window: raw_window_handle::RawWindowHandle)
                                              -> Result<Self::NativeWidget, Error>;
}
