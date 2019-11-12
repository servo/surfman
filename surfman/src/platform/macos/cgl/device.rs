// surfman/surfman/src/platform/macos/cgl/device.rs
//
//! A handle to the device. (This is a no-op, because handles are implicit in Apple's Core OpenGL.)

use crate::GLApi;
use crate::platform::macos::system::device::{Adapter as SystemAdapter, Device as SystemDevice};
use super::connection::Connection;

/// Represents a display adapter on macOS.
/// 
/// Adapters can be sent between threads. You can use them with a `Connection` to open the device.
#[derive(Clone, Debug)]
pub struct Adapter(pub(crate) SystemAdapter);

#[derive(Clone)]
pub struct Device(pub(crate) SystemDevice);

impl Device {
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection(self.0.connection())
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter(self.0.adapter())
    }

    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }
}
