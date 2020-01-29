// surfman/surfman/src/platform/macos/system/device.rs
//
//! A handle to the device. (This is a no-op, because handles are implicit in `IOSurface`.)

use crate::Error;
use super::connection::Connection;

use std::marker::PhantomData;

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub struct Adapter {
    pub(crate) is_low_power: bool,
}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
#[derive(Clone)]
pub struct Device {
    adapter: Adapter,
    phantom: PhantomData<*mut ()>,
}

/// An empty type representing the native device.
///
/// Since display server connections are implicit on macOS, this type doesn't contain anything.
#[derive(Clone)]
pub struct NativeDevice;

impl Device {
    #[inline]
    pub(crate) fn new(adapter: Adapter) -> Result<Device, Error> {
        Ok(Device { adapter, phantom: PhantomData })
    }

    /// Returns the native device corresponding to this device.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }
}
