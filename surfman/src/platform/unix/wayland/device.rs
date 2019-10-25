// surfman/surfman/src/platform/unix/wayland/device.rs
//
//! A wrapper around Wayland `EGLDisplay`s.

use crate::egl;
use crate::glx::types::Display as GlxDisplay;
use crate::platform::generic::egl::device::{NativeDisplay, OwnedEGLDisplay, UnsafeEGLDisplayRef};
use crate::{Error, GLApi};
use super::adapter::Adapter;
use super::connection::{Connection, NativeConnection};

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
    pub(crate) native_connection: Box<dyn NativeConnection>,
}

impl Device {
    #[inline]
    pub fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        unsafe {
            let egl_display =
                egl::GetDisplay(connection.native_connection.wayland_display() as *const _);
            if egl_display == egl::NO_DISPLAY {
                return Err(Error::DeviceOpenFailed);
            }

            let (mut egl_major_version, mut egl_minor_version) = (0, 0);
            let ok = egl::Initialize(egl_display, &mut egl_major_version, &mut egl_minor_version);
            assert_ne!(ok, egl::FALSE);

            Ok(Device {
                native_display: Box::new(OwnedEGLDisplay { egl_display }),
                native_connection: connection.native_connection.retain(),
            })
        }
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
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }
}
