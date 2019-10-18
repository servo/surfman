//! A device abstraction that can switch between hardware and software rendering.

use crate::{Error, GLApi};
use crate::platform::default::device::Device as HWDevice;
use crate::platform::generic::osmesa::device::Device as OSMesaDevice;
use super::adapter::Adapter;

pub enum Device {
    Hardware(HWDevice),
    Software(OSMesaDevice),
}

impl Device {
    #[inline]
    pub fn new(adapter: &Adapter) -> Result<Device, Error> {
        match *adapter {
            Adapter::Hardware(ref adapter) => HWDevice::new(adapter).map(Device::Hardware),
            Adapter::Software(ref adapter) => OSMesaDevice::new(adapter).map(Device::Software),
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        match *self {
            Device::Hardware(ref device) => Adapter::Hardware(device.adapter()),
            Device::Software(ref device) => Adapter::Software(device.adapter()),
        }
    }

    // FIXME(pcwalton): This should take `self`!
    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }
}
