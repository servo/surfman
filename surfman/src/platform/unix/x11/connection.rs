// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).
//!
//! FIXME(pcwalton): I think this should actually wrap the `Display`.

use crate::error::Error;
use super::adapter::Adapter;
use super::surface::NativeWidget;

use std::ffi::CString;

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

#[derive(Clone)]
pub struct Connection {
    pub(crate) display_name: Option<CString>,
}

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection { display_name: None })
    }

    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The X11 backend has no software support, so this returns an error.
    ///
    /// TODO(pcwalton): If Mesa is in use, maybe we could use `llvmpipe` somehow?
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        if window.get_xlib_display().is_some() {
            Connection::new()
        } else {
            Err(Error::IncompatibleWinitWindow)
        }
    }

    #[cfg(feature = "sm-winit")]
    pub fn create_native_widget_from_winit_window(&self, window: &Window)
                                                  -> Result<NativeWidget, Error> {
        match window.get_xlib_window() {
            Some(window) => Ok(NativeWidget { window }),
            None => Err(Error::IncompatibleNativeWidget),
        }
    }
}
