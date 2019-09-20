//! A handle to the device. (This is a no-op in OSMesa.)

use crate::{Error, GLApi};
use super::adapter::Adapter;

use std::marker::PhantomData;

#[derive(Clone)]
pub struct Device {
    pub(crate) phantom: PhantomData<*mut ()>,
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

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }
}
