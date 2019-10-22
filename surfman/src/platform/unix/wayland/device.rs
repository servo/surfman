// surfman/surfman/src/platform/unix/wayland/device.rs
//
//! A wrapper around Wayland `EGLDisplay`s.

use crate::egl;
use crate::glx::types::Display as GlxDisplay;
use crate::platform::generic::egl::device::{NativeDisplay, OwnedEGLDisplay, UnsafeEGLDisplayRef};
use crate::{Error, GLApi};
use super::adapter::{Adapter, NativeAdapter};

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
    pub(crate) native_adapter: Box<dyn NativeAdapter>,
}

impl Device {
    #[inline]
    pub fn new(adapter: &Adapter) -> Result<Device, Error> {
        unsafe {
            let native_display = egl::GetDisplay(adapter.native_adapter.wayland_display());
            if native_display == egl::NO_DISPLAY {
                return Err(Error::DeviceOpenFailed);
            }
            Ok(Device {
                native_display: Box::new(OwnedEGLDisplay { egl_display }),
                native_adapter: adapter.native_adapter.clone(),
            })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter { native_adapter: self.native_adapter.clone() }
    }

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }
}
