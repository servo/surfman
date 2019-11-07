// surfman/surfman/src/platform/unix/wayland/device.rs
//
//! A wrapper around Wayland `EGLDisplay`s.

use crate::{Error, GLApi};
use super::adapter::Adapter;
use super::connection::{Connection, NativeConnection};

pub struct Device {
    pub(crate) native_connection: Box<dyn NativeConnection>,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection) -> Result<Device, Error> {
        Ok(Device { native_connection: connection.native_connection.retain() })
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        Connection { native_connection: self.native_connection.retain() }
    }

    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }
}
