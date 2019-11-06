// surfman/surfman/src/implementation/connection.rs
//
//! This is an included private module that automatically produces the implementation of the
//! `Connection` trait for a backend.

use crate::Error;
use crate::connection::Connection as ConnectionInterface;
use super::super::adapter::Adapter;
use super::super::connection::Connection;
use super::super::device::Device;
use super::super::surface::NativeWidget;

#[cfg(feature = "sm-winit")]
use winit::Window;

impl ConnectionInterface for Connection {
    type Adapter = Adapter;
    type Device = Device;
    type NativeWidget = NativeWidget;

    #[inline]
    fn new() -> Result<Connection, Error> {
        Connection::new()
    }

    #[inline]
    fn create_adapter(&self) -> Result<Adapter, Error> {
        Connection::create_adapter(self)
    }

    #[inline]
    fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Connection::create_hardware_adapter(self)
    }

    #[inline]
    fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Connection::create_software_adapter(self)
    }

    #[inline]
    fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Connection::create_device(self, adapter)
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        Connection::from_winit_window(window)
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn create_native_widget_from_winit_window(&self, window: &Window)
                                              -> Result<NativeWidget, Error> {
        Connection::create_native_widget_from_winit_window(self, window)
    }
}
