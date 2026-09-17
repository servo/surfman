//! A connection abstraction that allows the choice of backends dynamically.

use super::device::{Adapter, Device};
use super::surface::NativeWidget;
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;

use std::os::raw::c_void;

/// A connection to the display server.
pub enum Connection<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: ConnectionInterface,
    Alt::Connection: ConnectionInterface,
{
    /// The default connection to the display server.
    Default(Def::Connection),
    /// The alternate connection to the display server.
    Alternate(Alt::Connection),
}

impl<Def, Alt> Clone for Connection<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: Clone,
    Alt::Connection: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Connection::Default(ref connection) => Connection::Default(connection.clone()),
            Connection::Alternate(ref connection) => Connection::Alternate(connection.clone()),
        }
    }
}

impl<Def, Alt> Connection<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: ConnectionInterface<Device = Def>,
    Alt::Connection: ConnectionInterface<Device = Alt>,
{
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection<Def, Alt>, Error> {
        match <Def::Connection>::new() {
            Ok(connection) => Ok(Connection::Default(connection)),
            Err(_) => <Alt::Connection>::new().map(Connection::Alternate),
        }
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    pub fn gl_api(&self) -> GLApi {
        match *self {
            Connection::Default(ref connection) => connection.gl_api(),
            Connection::Alternate(ref connection) => connection.gl_api(),
        }
    }

    /// Returns the "best" adapter on this system.
    ///
    /// This is an alias for `Connection::create_hardware_adapter()`.
    pub fn create_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_adapter().map(Adapter::Alternate)
            }
        }
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    pub fn create_hardware_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_hardware_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_hardware_adapter().map(Adapter::Alternate)
            }
        }
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    pub fn create_low_power_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_low_power_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => connection
                .create_low_power_adapter()
                .map(Adapter::Alternate),
        }
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
    pub fn create_software_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_software_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_software_adapter().map(Adapter::Alternate)
            }
        }
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    pub fn create_device(&self, adapter: &Adapter<Def, Alt>) -> Result<Device<Def, Alt>, Error> {
        match (self, adapter) {
            (Connection::Default(connection), Adapter::Default(adapter)) => {
                connection.create_device(adapter).map(Device::Default)
            }
            (Connection::Alternate(connection), Adapter::Alternate(adapter)) => {
                connection.create_device(adapter).map(Device::Alternate)
            }
            _ => Err(Error::IncompatibleAdapter),
        }
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle")]
    pub fn from_display_handle(
        handle: raw_window_handle::DisplayHandle,
    ) -> Result<Connection<Def, Alt>, Error> {
        match <Def::Connection>::from_display_handle(handle) {
            Ok(connection) => Ok(Connection::Default(connection)),
            Err(_) => <Alt::Connection>::from_display_handle(handle).map(Connection::Alternate),
        }
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        size: Size2D<i32>,
    ) -> NativeWidget<Def, Alt> {
        match *self {
            Connection::Default(ref connection) => {
                NativeWidget::Default(connection.create_native_widget_from_ptr(raw, size))
            }
            Connection::Alternate(ref connection) => {
                NativeWidget::Alternate(connection.create_native_widget_from_ptr(raw, size))
            }
        }
    }

    /// Create a native widget type from the given `WindowHandle`.
    #[cfg(feature = "sm-raw-window-handle")]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: raw_window_handle::WindowHandle,
        size: Size2D<i32>,
    ) -> Result<NativeWidget<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => connection
                .create_native_widget_from_window_handle(handle, size)
                .map(NativeWidget::Default),
            Connection::Alternate(ref connection) => connection
                .create_native_widget_from_window_handle(handle, size)
                .map(NativeWidget::Alternate),
        }
    }
}

impl<Def, Alt> ConnectionInterface for Connection<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: ConnectionInterface<Device = Def>,
    Alt::Connection: ConnectionInterface<Device = Alt>,
{
    type Adapter = Adapter<Def, Alt>;
    type Device = Device<Def, Alt>;
    type NativeWidget = NativeWidget<Def, Alt>;

    #[inline]
    fn new() -> Result<Connection<Def, Alt>, Error> {
        Connection::new()
    }

    #[inline]
    fn gl_api(&self) -> GLApi {
        Connection::gl_api(self)
    }

    #[inline]
    fn create_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_adapter(self)
    }

    #[inline]
    fn create_hardware_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_hardware_adapter(self)
    }

    #[inline]
    fn create_low_power_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_low_power_adapter(self)
    }

    #[inline]
    fn create_software_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_software_adapter(self)
    }

    #[inline]
    fn create_device(&self, adapter: &Adapter<Def, Alt>) -> Result<Device<Def, Alt>, Error> {
        Connection::create_device(self, adapter)
    }

    #[cfg(feature = "sm-raw-window-handle")]
    fn from_display_handle(
        handle: raw_window_handle::DisplayHandle,
    ) -> Result<Connection<Def, Alt>, Error> {
        Connection::from_display_handle(handle)
    }

    #[inline]
    unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        size: Size2D<i32>,
    ) -> NativeWidget<Def, Alt> {
        Connection::create_native_widget_from_ptr(self, raw, size)
    }

    #[cfg(feature = "sm-raw-window-handle")]
    fn create_native_widget_from_window_handle(
        &self,
        handle: raw_window_handle::WindowHandle,
        size: Size2D<i32>,
    ) -> Result<Self::NativeWidget, Error> {
        Connection::create_native_widget_from_window_handle(self, handle, size)
    }
}
