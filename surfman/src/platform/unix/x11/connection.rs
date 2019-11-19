// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).

use crate::error::Error;
use crate::platform::unix::generic::device::Adapter;
use super::device::{Device, NativeDevice};
use super::surface::NativeWidget;

use std::ptr;
use std::sync::Arc;
use x11::xlib::{Display, XCloseDisplay, XOpenDisplay};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

/// A connection to the X11 display server.
#[derive(Clone)]
pub struct Connection {
    pub(crate) display_holder: Arc<DisplayHolder>,
}

unsafe impl Send for Connection {}

pub(crate) struct DisplayHolder {
    pub(crate) display: *mut Display,
    pub(crate) display_is_owned: bool,
}

/// Wrapper for an X11 `Display`.
#[derive(Clone)]
pub struct NativeConnection(pub *mut Display);

impl Drop for DisplayHolder {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if self.display_is_owned {
                XCloseDisplay(self.display);
            }
            self.display = ptr::null_mut();
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
            Ok(Connection {
                display_holder: Arc::new(DisplayHolder { display, display_is_owned: true }),
            })
        }
    }

    /// Wraps an existing X11 `Display` in a `Connection`.
    ///
    /// The display is not retained, as there is no way to do that in the X11 API. Therefore, it is
    /// the caller's responsibility to ensure that the display connection is not closed before this
    /// `Connection` object is disposed of.
    #[inline]
    pub unsafe fn from_native_connection(native_connection: NativeConnection)
                                         -> Result<Connection, Error> {
        Ok(Connection {
            display_holder: Arc::new(DisplayHolder {
                display: native_connection.0,
                display_is_owned: false,
            }),
        })
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection(self.display_holder.display)
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

    /// Opens the hardware device corresponding to the adapter wrapped in the given native
    /// device.
    ///
    /// This is present for compatibility with other backends.
    #[inline]
    pub fn create_device_from_native_device(&self, native_device: NativeDevice)
                                            -> Result<Device, Error> {
        Device::new(self, &native_device.adapter)
    }

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        unsafe {
            if let Some(display) = window.get_xlib_display() {
                Connection::from_native_connection(NativeConnection(display as *mut Display))
            } else {
                Err(Error::IncompatibleWinitWindow)
            }
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

