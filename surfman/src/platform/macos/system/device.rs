// surfman/surfman/src/platform/macos/system/device.rs
//
//! A handle to the device. (This is a no-op, because handles are implicit in `IOSurface`.)

use super::connection::Connection;
use crate::Error;

use metal::Device as MetalDevice;
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

/// The Metal device corresponding to this device.
#[derive(Clone)]
pub struct NativeDevice(pub MetalDevice);

impl Device {
    #[inline]
    pub(crate) fn new(adapter: Adapter) -> Result<Device, Error> {
        Ok(Device {
            adapter,
            phantom: PhantomData,
        })
    }

    /// Returns the native device corresponding to this device.
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice(
            MetalDevice::all()
                .into_iter()
                .filter(|device| device.is_low_power() == self.adapter.is_low_power)
                .next()
                .expect("No Metal device found!"),
        )
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
