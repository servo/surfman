//! A thread-local handle to the device.

use crate::context::ContextID;
use crate::egl::types::{EGLAttrib, EGLBoolean, EGLConfig, EGLContext, EGLDisplay};
use crate::egl::types::{EGLSurface, EGLenum, EGLint};
use crate::{Error, GLApi, egl};
use super::adapter::Adapter;

use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr;

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
    pub fn new(adapter: &Adapter) -> Result<Device, Error> {
        unsafe {
            let display = egl::GetDisplay(egl::DEFAULT_DISPLAY);
            assert_ne!(egl_display, egl::NO_DISPLAY);
            let native_display = Box::new(OwnedEGLDisplay { egl_display });

            // I don't think this should ever fail.
            let (mut major_version, mut minor_version) = (0, 0);
            let result = egl::Initialize(native_display.egl_display(),
                                         &mut major_version,
                                         &mut minor_version);
            assert_ne!(result, egl::FALSE);

            Ok(Device { native_display })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GLES
    }
}

unsafe fn lookup_egl_extension(name: &'static [u8]) -> *mut c_void {
    let f = egl::GetProcAddress(&name[0] as *const u8 as *const c_char);
    assert_ne!(f as usize, 0);
    f as *mut c_void
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
        assert!(!self.is_destroyed());
        let result = egl::Terminate(self.egl_display);
        assert_ne!(result, egl::FALSE);
        self.egl_display = egl::NO_DISPLAY;
    }
}

pub(crate) struct UnsafeEGLDisplayRef {
    egl_display: EGLDisplay,
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
