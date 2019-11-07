// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).
//!
//! FIXME(pcwalton): I think this should actually wrap the `Display`.

use crate::error::Error;
use super::adapter::Adapter;
use super::device::Device;
use super::surface::NativeWidget;

use std::ptr;
use std::sync::Arc;
use x11::xlib::{Display, XCloseDisplay, XOpenDisplay};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

pub struct Connection {
    pub(crate) native_display: Box<dyn NativeDisplay>,
    pub(crate) quirks: Quirks,
}

unsafe impl Send for Connection {}

pub(crate) trait NativeDisplay {
    fn display(&self) -> *mut Display;
    fn is_destroyed(&self) -> bool;
    fn retain(&self) -> Box<dyn NativeDisplay>;
    unsafe fn destroy(&mut self);
}

bitflags! {
    pub struct Quirks: u8 {
        const BROKEN_GLX_TEXTURE_FROM_PIXMAP = 0x01;
    }
}

impl Clone for Connection {
    fn clone(&self) -> Connection {
        Connection {
            native_display: self.native_display.retain(),
            quirks: self.quirks.clone(),
        }
    }
}

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(Error::ConnectionFailed);
            }
            let display = Some(Arc::new(OwnedDisplay(display)));
            Ok(Connection {
                native_display: Box::new(SharedDisplay { display }),
                quirks: Quirks::detect(),
            })
        }
    }

    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::Hardware)
    }

    /// Returns the "best" low-power hardware adapter on this system.
    ///
    /// TODO(pcwalton): Use DRI PRIME if possible.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::Hardware)
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The X11 backend has no software support, so this returns an error.
    ///
    /// TODO(pcwalton): If Mesa is in use, maybe we could use `llvmpipe` somehow?
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::Software)
    }

    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(self, adapter)
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        if let Some(display) = window.get_xlib_display() {
            Ok(Connection {
                native_display: Box::new(UnsafeDisplayRef { display: display as *mut Display }),
                quirks: Quirks::detect(),
            })
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

pub(crate) struct SharedDisplay {
    pub(crate) display: Option<Arc<OwnedDisplay>>,
}

pub(crate) struct OwnedDisplay(*mut Display);

impl NativeDisplay for SharedDisplay {
    #[inline]
    fn display(&self) -> *mut Display {
        debug_assert!(!self.is_destroyed());
        self.display.as_ref().unwrap().0
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.display.is_none()
    }

    #[inline]
    fn retain(&self) -> Box<dyn NativeDisplay> {
        Box::new(SharedDisplay { display: self.display.clone() })
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.display = None;
    }
}

impl Drop for OwnedDisplay {
    fn drop(&mut self) {
        unsafe {
            XCloseDisplay(self.0);
        }
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

    #[inline]
    fn retain(&self) -> Box<dyn NativeDisplay> {
        Box::new(UnsafeDisplayRef { display: self.display })
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

