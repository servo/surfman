// surfman/surfman/src/platform/src/osmesa/connection.rs
//
//! A no-op connection. OSMesa needs no connection, as it is a CPU-based off-screen-only API.

use crate::Error;
use super::device::{Adapter, Device};
use super::surface::NativeWidget;

#[cfg(feature = "sm-winit")]
use winit::Window;

/// A no-op connection.
#[derive(Clone)]
pub struct Connection;

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection)
    }

    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Returns the "best" hardware adapter on this system.
    ///
    /// OSMesa is a software backend, so this returns an error.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Err(Error::NoHardwareAdapters)
    }

    /// Returns the "best" software adapter on this system.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    #[inline]
    pub fn create_device(&self, _: &Adapter) -> Result<Device, Error> {
        Device::new()
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Err(Error::IncompatibleNativeWidget)
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn create_native_widget_from_winit_window(&self, _: &Window) -> Result<NativeWidget, Error> {
        Err(Error::IncompatibleNativeWidget)
    }
}
