//! A wrapper around X11 Displays.

use crate::glx::types::Display as GlxDisplay;
use crate::{Error, GLApi};
use super::adapter::Adapter;
use super::connection::Connection;

use std::ffi::CStr;
use std::os::raw::c_int;
use std::ptr;
use x11::xlib::{self, Display, XCloseDisplay, XDisplayString, XOpenDisplay};

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
    pub(crate) quirks: Quirks,
}

pub(crate) trait NativeDisplay {
    fn display(&self) -> *mut Display;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

bitflags! {
    pub struct Quirks: u8 {
        const BROKEN_GLX_TEXTURE_FROM_PIXMAP = 0x01;
    }
}

impl Device {
    #[inline]
    pub fn new(connection: &Connection, _: &Adapter) -> Result<Device, Error> {
        unsafe {
            let display_name = match connection.display_name {
                None => ptr::null(),
                Some(ref display_name) => display_name.as_ptr(),
            };
            let display = XOpenDisplay(display_name);
            if display.is_null() {
                return Err(Error::DeviceOpenFailed);
            }
            Ok(Device {
                native_display: Box::new(OwnedDisplay { display }),
                quirks: Quirks::detect(),
            })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        unsafe {
            let display_name = XDisplayString(self.native_display.display());
            assert!(!display_name.is_null());
            Connection { display_name: Some(CStr::from_ptr(display_name).to_owned()) }
        }
    }

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }

    pub(crate) fn glx_display(&self) -> *mut GlxDisplay {
        self.native_display.display() as *mut GlxDisplay
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

impl Quirks {
    pub(crate) fn detect() -> Quirks {
        // TODO(pcwalton): Whitelist implementations with working `GLX_texture_from_pixmap`.
        Quirks::BROKEN_GLX_TEXTURE_FROM_PIXMAP
    }
}

