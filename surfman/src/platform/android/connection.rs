// surfman/surfman/src/platform/android/connection.rs
//
//! A no-op connection for Android.
//! 
//! FIXME(pcwalton): Should this instead wrap `EGLDisplay`? Is that thread-safe on Android?

use crate::Error;
use super::device::{Adapter, Device};

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
        self.create_hardware_adapter()
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The Android backend has no software support, so this returns an error.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }

    /// Opens a device.
    #[inline]
    pub fn create_device(&self, _: &Adapter) -> Result<Device, Error> {
        Device::new()
    }
}
