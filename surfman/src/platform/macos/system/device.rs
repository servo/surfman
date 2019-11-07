// surfman/surfman/src/platform/macos/system/device.rs
//
//! A handle to the device. (This is a no-op, because handles are implicit in `IOSurface`.)

use crate::Error;
use super::adapter::Adapter;
use super::connection::Connection;

use std::marker::PhantomData;

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
