//! A handle to the device. (This is a no-op, because handles are implicit in Apple's Core OpenGL.)

use crate::Error;
use std::marker::PhantomData;

#[cfg(feature = "sm-glutin")]
use glutin::Window;

#[derive(Clone)]
pub struct Device {
    phantom: PhantomData<*mut ()>,
}

impl Device {
    #[inline]
    pub fn new() -> Result<Device, Error> {
        Ok(Device { phantom: PhantomData })
    }

    #[cfg(feature = "sm-glutin")]
    #[inline]
    pub fn from_glutin_window(_: &Window) -> Result<Device, Error> {
        // Core OpenGL automatically manages connections to the window server, so there's nothing
        // to do here.
        Device::new()
    }
}
