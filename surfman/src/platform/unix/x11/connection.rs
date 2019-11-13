// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).

use crate::error::Error;
use crate::platform::unix::generic::device::Adapter;
use super::device::Device;
use super::surface::NativeWidget;

use std::ptr;
use std::sync::Arc;
use x11::xlib::{Display, XCloseDisplay, XOpenDisplay};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

/// A connection to the X11 display server.
pub struct Connection {
    pub(crate) native_display: Box<dyn NativeDisplay>,
}

unsafe impl Send for Connection {}

pub(crate) trait NativeDisplay {
    fn display(&self) -> *mut Display;
    fn is_destroyed(&self) -> bool;
    fn retain(&self) -> Box<dyn NativeDisplay>;
    unsafe fn destroy(&mut self);
}

impl Clone for Connection {
    fn clone(&self) -> Connection {
        Connection { native_display: self.native_display.retain() }
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
            Ok(Connection { native_display: Box::new(SharedDisplay { display }) })
        }
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    /// 
    /// This is an alias for `Connection::create_hardware_adapter()`.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::hardware())
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::low_power())
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::software())
    }

    /// Opens the hardware device corresponding to the given adapter.
    /// 
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(self, adapter)
    }

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        if let Some(display) = window.get_xlib_display() {
            Ok(Connection {
                native_display: Box::new(UnsafeDisplayRef { display: display as *mut Display }),
            })
        } else {
            Err(Error::IncompatibleWinitWindow)
        }
    }

    /// Creates a native widget type from the given `winit` window.
    /// 
    /// This type can be later used to create surfaces that render to the window.
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

