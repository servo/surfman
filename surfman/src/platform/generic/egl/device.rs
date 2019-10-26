// surfman/surfman/src/platform/generic/egl/device.rs
//
//! Functionality common to backends using EGL displays.

use crate::egl::types::EGLDisplay;
use crate::egl;

use std::mem;
use std::os::raw::{c_char, c_void};

pub(crate) trait NativeDisplay {
    fn egl_display(&self) -> EGLDisplay;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
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

pub(crate) unsafe fn lookup_egl_extension(name: &'static [u8]) -> *mut c_void {
    mem::transmute(egl::GetProcAddress(&name[0] as *const u8 as *const c_char))
}
