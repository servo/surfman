//! A thread-local handle to the device.

use crate::egl::types::EGLDisplay;
use crate::egl;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::{Error, GLApi};
use super::adapter::Adapter;
use super::connection::Connection;

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
}

pub(crate) trait NativeDisplay {
    fn egl_display(&self) -> EGLDisplay;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

impl Device {
    #[inline]
    pub fn new(_: &Connection, _: &Adapter) -> Result<Device, Error> {
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                let egl_display = egl.GetDisplay(egl::DEFAULT_DISPLAY);
                assert_ne!(egl_display, egl::NO_DISPLAY);
                let native_display = Box::new(OwnedEGLDisplay { egl_display });

                // I don't think this should ever fail.
                let (mut major_version, mut minor_version) = (0, 0);
                let result = egl.Initialize(native_display.egl_display(),
                                            &mut major_version,
                                            &mut minor_version);
                assert_ne!(result, egl::FALSE);

                Ok(Device { native_display })
            }
        })
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }
}

pub(crate) struct OwnedEGLDisplay {
    pub(crate) egl_display: EGLDisplay,
}

impl NativeDisplay for OwnedEGLDisplay {
    #[inline]
    fn egl_display(&self) -> EGLDisplay {
        debug_assert!(!self.is_destroyed());
        self.egl_display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_display == egl::NO_DISPLAY
    }

    unsafe fn destroy(&mut self) {
        EGL_FUNCTIONS.with(|egl| {
            assert!(!self.is_destroyed());
            let result = egl.Terminate(self.egl_display);
            assert_ne!(result, egl::FALSE);
            self.egl_display = egl::NO_DISPLAY;
        })
    }
}

pub(crate) struct UnsafeEGLDisplayRef {
    pub(crate) egl_display: EGLDisplay,
}

impl NativeDisplay for UnsafeEGLDisplayRef {
    #[inline]
    fn egl_display(&self) -> EGLDisplay {
        debug_assert!(!self.is_destroyed());
        self.egl_display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_display == egl::NO_DISPLAY
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.egl_display = egl::NO_DISPLAY;
    }
}
