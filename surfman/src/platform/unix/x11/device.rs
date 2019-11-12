//! A wrapper around X11 Displays.

use crate::glx::types::Display as GlxDisplay;
use crate::{Error, GLApi};
use super::adapter::Adapter;
use super::connection::Connection;

use std::os::raw::c_int;
use std::ptr;
use x11::xlib::{self, Display, XCloseDisplay};

pub struct Device {
    pub(crate) connection: Connection,
}

pub(crate) trait NativeDisplay {
    fn display(&self) -> *mut Display;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

impl Device {
    #[inline]
    pub fn new(connection: &Connection, _: &Adapter) -> Result<Device, Error> {
        Ok(Device { connection: (*connection).clone() })
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        self.connection.clone()
    }

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }

    pub(crate) fn glx_display(&self) -> *mut GlxDisplay {
        self.connection.native_display.display() as *mut GlxDisplay
    }
}

pub(crate) struct OwnedDisplay {
    pub(crate) display: *mut Display,
}

impl NativeDisplay for OwnedDisplay {
    #[inline]
    fn display(&self) -> *mut Display {
        debug_assert!(!self.is_destroyed());
        self.display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.display.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        let result = XCloseDisplay(self.display);
        assert_eq!(result, xlib::Success as c_int);
        self.display = ptr::null_mut();
    }
}

pub(crate) struct UnsafeDisplayRef {
    pub(crate) display: *mut Display,
}

impl NativeDisplay for UnsafeDisplayRef {
    #[inline]
    fn display(&self) -> *mut Display {
        debug_assert!(!self.is_destroyed());
        self.display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.display.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.display = ptr::null_mut();
    }
}

