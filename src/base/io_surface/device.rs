//! A handle to the device. (This is a no-op, because handles are implicit in `IOSurface`.)

use super::adapter::AppleAdapter;
use super::connection::Connection;
use crate::Error;

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLCopyAllDevices, MTLDevice};
use std::marker::PhantomData;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
#[derive(Clone)]
pub struct Device {
    pub(crate) adapter: AppleAdapter,
    phantom: PhantomData<*mut ()>,
}

/// The Metal device corresponding to this device.
#[derive(Clone)]
pub struct NativeDevice(pub Retained<ProtocolObject<dyn MTLDevice>>);

impl Device {
    #[inline]
    pub(crate) fn new(adapter: AppleAdapter) -> Result<Device, Error> {
        Ok(Device {
            adapter,
            phantom: PhantomData,
        })
    }

    /// Returns the native device corresponding to this device.
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice(
            MTLCopyAllDevices()
                .into_iter()
                .find(|device| device.isLowPower() == self.adapter.is_low_power)
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
    pub fn adapter(&self) -> AppleAdapter {
        self.adapter.clone()
    }
}
