//! A handle to the device. (This is a no-op, because handles are implicit in Apple's Core OpenGL.)

use crate::Error;
use super::adapter::Adapter;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Device {
    phantom: PhantomData<*mut ()>,
}

impl Device {
    #[inline]
    pub fn new(_: &Adapter) -> Result<Device, Error> {
        Ok(Device { phantom: PhantomData })
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }
}
