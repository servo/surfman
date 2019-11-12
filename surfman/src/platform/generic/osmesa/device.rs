// surfman/surfman/src/platform/generic/osmesa/device.rs
//
//! A handle to the device. (This is a no-op in OSMesa.)

use crate::{Error, GLApi};
use super::connection::Connection;

use std::marker::PhantomData;

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub struct Adapter;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
#[derive(Clone)]
pub struct Device {
    pub(crate) phantom: PhantomData<*mut ()>,
}

impl Device {
    #[inline]
    pub(crate) fn new() -> Result<Device, Error> {
        Ok(Device { phantom: PhantomData })
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }
}
