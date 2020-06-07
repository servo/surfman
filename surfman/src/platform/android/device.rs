// surfman/surfman/src/platform/android/device.rs
//
//! A thread-local handle to the device.

use super::connection::Connection;
use crate::egl;
use crate::egl::types::EGLDisplay;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::{Error, GLApi};

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub struct Adapter;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) egl_display: EGLDisplay,
    pub(crate) display_is_owned: bool,
}

/// Wrapper for an `EGLDisplay`.
#[derive(Clone, Copy)]
pub struct NativeDevice(pub EGLDisplay);

impl Drop for Device {
    fn drop(&mut self) {
        EGL_FUNCTIONS.with(|egl| unsafe {
            if !self.display_is_owned {
                return;
            }
            let result = egl.Terminate(self.egl_display);
            assert_ne!(result, egl::FALSE);
            self.egl_display = egl::NO_DISPLAY;
        })
    }
}

impl NativeDevice {
    /// Returns the current EGL display.
    ///
    /// If there is no current EGL display, `egl::NO_DISPLAY` is returned.
    pub fn current() -> NativeDevice {
        EGL_FUNCTIONS.with(|egl| unsafe { NativeDevice(egl.GetCurrentDisplay()) })
    }
}

impl Device {
    #[inline]
    pub(crate) fn new() -> Result<Device, Error> {
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                let egl_display = egl.GetDisplay(egl::DEFAULT_DISPLAY);
                assert_ne!(egl_display, egl::NO_DISPLAY);

                // I don't think this should ever fail.
                let (mut major_version, mut minor_version) = (0, 0);
                let result = egl.Initialize(egl_display, &mut major_version, &mut minor_version);
                assert_ne!(result, egl::FALSE);

                Ok(Device {
                    egl_display,
                    display_is_owned: true,
                })
            }
        })
    }

    /// Returns the EGL display corresponding to this device.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice(self.egl_display)
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }
}
