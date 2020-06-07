// surfman/surfman/src/platform/unix/generic/device.rs
//
//! A wrapper around surfaceless Mesa `EGLDisplay`s.

use super::connection::{Connection, NativeConnectionWrapper};
use crate::{Error, GLApi};

use std::env;
use std::sync::Arc;

static MESA_SOFTWARE_RENDERING_ENV_VAR: &'static str = "LIBGL_ALWAYS_SOFTWARE";
static MESA_DRI_PRIME_ENV_VAR: &'static str = "DRI_PRIME";

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub enum Adapter {
    #[doc(hidden)]
    Hardware,
    #[doc(hidden)]
    HardwarePrime,
    #[doc(hidden)]
    Software,
}

impl Adapter {
    #[inline]
    pub(crate) fn hardware() -> Adapter {
        Adapter::HardwarePrime
    }

    #[inline]
    pub(crate) fn low_power() -> Adapter {
        Adapter::Hardware
    }

    #[inline]
    pub(crate) fn software() -> Adapter {
        Adapter::Software
    }

    pub(crate) fn set_environment_variables(&self) {
        match *self {
            Adapter::Hardware | Adapter::HardwarePrime => {
                env::remove_var(MESA_SOFTWARE_RENDERING_ENV_VAR);
            }
            Adapter::Software => {
                env::set_var(MESA_SOFTWARE_RENDERING_ENV_VAR, "1");
            }
        }

        match *self {
            Adapter::Software => {}
            Adapter::Hardware => {
                env::remove_var(MESA_DRI_PRIME_ENV_VAR);
            }
            Adapter::HardwarePrime => {
                env::set_var(MESA_DRI_PRIME_ENV_VAR, "1");
            }
        }
    }
}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
    pub(crate) adapter: Adapter,
}

/// Wraps an adapter.
///
/// On Wayland, devices and adapters are essentially identical types.
#[derive(Clone)]
pub struct NativeDevice {
    /// The hardware adapter corresponding to this device.
    pub adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device {
            native_connection: connection.native_connection.clone(),
            adapter: (*adapter).clone(),
        })
    }

    /// Returns the native device corresponding to this device.
    ///
    /// This method is essentially an alias for the `adapter()` method on Mesa, since there is
    /// no explicit concept of a device on this backend.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice {
            adapter: self.adapter(),
        }
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection {
            native_connection: self.native_connection.clone(),
        }
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }
}
