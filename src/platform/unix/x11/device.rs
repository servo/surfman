//! A wrapper around X11 Displays.

use crate::Error;
use super::adapter::Adapter;
use std::marker::PhantomData;
use std::ptr;
use x11::xlib::{Display, XCloseDisplay, XOpenDisplay};

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
}

pub(crate) trait NativeDisplay {
    fn display(&self) -> Display;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

impl Device {
    #[inline]
    pub fn new(adapter: &Adapter) -> Result<Device, Error> {
        unsafe {
            let display_name = match adapter.display_name {
                None => ptr::null(),
                Some(ref display_name) => display_name.as_ptr(),
            };
            let display = XOpenDisplay(display_name);
            if display.is_null() {
                return Err(Error::DeviceOpenFailed);
            }
            Ok(Device { native_display: Box::new(OwnedDisplay { display }) })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        unsafe {
            let display_name = XDisplayString(self.native_display.display());
            assert!(!display_name.is_null());
            Adapter { adapter: Some(CStr::from_ptr(display_name).to_owned()) }
        }
    }
}

pub(crate) struct OwnedDisplay {
    pub(crate) display: Display,
}

impl NativeDisplay for OwnedDisplay {
    #[inline]
    fn display(&self) -> Display {
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
        assert_eq!(result, xlib::Success);
        self.display = ptr::null();
    }
}

pub(crate) struct UnsafeDisplayRef {
    pub(crate) display: Display,
}

impl NativeDisplay for UnsafeDisplayRef {
    #[inline]
    fn display(&self) -> Display {
        debug_assert!(!self.is_destroyed());
        self.display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.display.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.display = ptr::null();
    }
}
