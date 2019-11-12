// surfman/surfman/src/platform/unix/wayland/device.rs
//
//! A wrapper around Wayland `EGLDisplay`s.

use crate::{Error, GLApi};
use super::connection::{Connection, NativeConnection};

#[derive(Clone, Debug)]
pub enum Adapter {
    Hardware,
    Software,
}

pub struct Device {
    pub(crate) native_connection: Box<dyn NativeConnection>,
    pub(crate) adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device {
            native_connection: connection.native_connection.retain(),
            adapter: (*adapter).clone(),
        })
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
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
