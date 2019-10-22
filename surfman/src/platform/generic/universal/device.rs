//! A device abstraction that can switch between hardware and software rendering.

use crate::{Error, GLApi};
use crate::platform::default::device::Device as HWDevice;
use crate::platform::generic::osmesa::connection::Connection as OSMesaConnection;
use crate::platform::generic::osmesa::device::Device as OSMesaDevice;
use super::adapter::Adapter;
use super::connection::Connection;

pub enum Device {
    Hardware(HWDevice),
    Software(OSMesaDevice),
}

impl Device {
    #[inline]
    pub fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        match (connection, adapter) {
            (&Connection::Some(ref connection), &Adapter::Hardware(ref adapter)) => {
                HWDevice::new(connection, adapter).map(Device::Hardware)
            }
            (&Connection::None, &Adapter::Hardware(_)) => Err(Error::ConnectionRequired),
            (_, &Adapter::Software(ref adapter)) => {
                // TODO(pcwalton): Support platform window server connections with OSMesa.
                OSMesaDevice::new(&OSMesaConnection::new().unwrap(), adapter).map(Device::Software)
            }
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        match *self {
            Device::Hardware(ref device) => Adapter::Hardware(device.adapter()),
            Device::Software(ref device) => Adapter::Software(device.adapter()),
        }
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        match *self {
            Device::Hardware(ref device) => Connection::Some(device.connection()),
            Device::Software(_) => {
                // TODO(pcwalton): Support platform window server connections with OSMesa.
                Connection::None
            }
        }
    }

    // FIXME(pcwalton): This should take `self`!
    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }
}
