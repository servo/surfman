// surfman/surfman/src/platform/macos/cgl/device.rs
//
//! A handle to the device. (This is a no-op, because handles are implicit in Apple's Core OpenGL.)

use super::connection::Connection;
use crate::platform::macos::system::device::{Adapter as SystemAdapter, Device as SystemDevice};
use crate::GLApi;

pub use crate::platform::macos::system::device::NativeDevice;

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub struct Adapter(pub(crate) SystemAdapter);

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
#[derive(Clone)]
pub struct Device(pub(crate) SystemDevice);

impl Device {
    /// Returns the native device corresponding to this device.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        self.0.native_device()
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection(self.0.connection())
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter(self.0.adapter())
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }
}
