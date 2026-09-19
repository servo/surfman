//! A connection abstraction that allows the choice of backends dynamically.

use super::device::Device;
use super::surface::NativeWidget;
use crate::adapter::AdapterPreferences;
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use crate::GLApi;
use crate::{Adapter, Error};

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

    /// Returns an adapter on this system according to the provided preferences.
    pub fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error> {
        match *self {
            Self::Default(ref connection) => connection.create_adapter(preferences),
            Self::Alternate(ref connection) => connection.create_adapter(preferences),
        }
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device<Def, Alt>, Error> {
        match self {
            Self::Default(connection) => connection.create_device(adapter).map(Device::Default),
            Self::Alternate(connection) => connection.create_device(adapter).map(Device::Alternate),
        }
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
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
    fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error> {
        Connection::create_adapter(self, preferences)
    }

    #[inline]
    fn create_device(&self, adapter: &Adapter) -> Result<Device<Def, Alt>, Error> {
        Connection::create_device(self, adapter)
    }

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

    fn create_native_widget_from_window_handle(
        &self,
        handle: raw_window_handle::WindowHandle,
        size: Size2D<i32>,
    ) -> Result<Self::NativeWidget, Error> {
        Connection::create_native_widget_from_window_handle(self, handle, size)
    }
}
