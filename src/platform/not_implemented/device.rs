//! A stub implementation of a device that reports errors when methods are invoked on it.

use crate::Error;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Device {
    phantom: PhantomData<*mut ()>,
}

impl Device {
    #[inline]
    pub fn new() -> Result<Device, Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }

    #[cfg(feature = "sm-glutin")]
    #[inline]
    pub fn from_glutin_window(_: &Window) -> Result<Device, Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }
}
