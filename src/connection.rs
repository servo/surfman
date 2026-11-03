//! The abstract interface that all connections conform to.

use crate::adapter::AdapterPreferences;
use crate::Adapter;
use crate::Device;
use crate::Error;
use crate::GLApi;

/// Methods relating to display server connections.
pub trait Connection: Sized {
    /// Connects to the default display.
    fn new() -> Result<Self, Error>;

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    fn gl_api(&self) -> GLApi;

    /// Returns an adapter on this system according to the provided preferences.
    fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error>;

    /// Opens a device.
    fn create_device(&self, adapter: &Adapter) -> Result<Device, Error>;

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    fn from_display_handle(handle: raw_window_handle::DisplayHandle) -> Result<Self, Error>;
}
