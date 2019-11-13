// surfman/surfman/src/connection.rs
//
//! The abstract interface that all connections conform to.

use crate::Error;

#[cfg(feature = "sm-winit")]
use winit::Window;

/// Methods relating to display server connections.
pub trait Connection: Sized {
    /// The adapter type associated with this connection.
    type Adapter;
    /// The device type associated with this connection.
    type Device;
    /// The native widget type associated with this connection.
    type NativeWidget;

    /// Connects to the default display.
    fn new() -> Result<Self, Error>;

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

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Self, Error>;

    /// Creates a native widget type from the given `winit` window.
    /// 
    /// This type can be later used to create surfaces that render to the window.
    #[cfg(feature = "sm-winit")]
    fn create_native_widget_from_winit_window(&self, window: &Window)
                                              -> Result<Self::NativeWidget, Error>;
}
