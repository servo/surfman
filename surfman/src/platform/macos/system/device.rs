// surfman/surfman/src/platform/macos/system/device.rs
//
//! A handle to the device. (This is a no-op, because handles are implicit in `IOSurface`.)

use crate::Error;
use super::connection::Connection;

use std::marker::PhantomData;

/// An adapter.
#[derive(Clone, Debug)]
pub struct Adapter {
    pub(crate) is_low_power: bool,
}

#[derive(Clone)]
pub struct Device {
    adapter: Adapter,
    phantom: PhantomData<*mut ()>,
}

impl Device {
    #[inline]
    pub(crate) fn new(adapter: Adapter) -> Result<Device, Error> {
        Ok(Device { adapter, phantom: PhantomData })
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }
}
