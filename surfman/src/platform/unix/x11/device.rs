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
    pub(crate) connection: Connection,
    pub(crate) adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device { connection: (*connection).clone(), adapter: (*adapter).clone() })
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        self.connection.clone()
    }

    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    pub(crate) fn glx_display(&self) -> *mut GlxDisplay {
        self.connection.native_display.display() as *mut GlxDisplay
    }
}

