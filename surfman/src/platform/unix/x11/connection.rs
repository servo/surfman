// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).

use crate::egl::types::{EGLAttrib, EGLDisplay};
use crate::egl;
use crate::error::Error;
use crate::glx::types::Display as GlxDisplay;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_PLATFORM_X11_KHR;
use crate::platform::unix::generic::device::Adapter;
use super::device::{Device, NativeDevice};
use super::surface::NativeWidget;

use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use std::sync::Arc;
use x11::xlib::{Display, XCloseDisplay, XInitThreads, XLockDisplay, XOpenDisplay, XUnlockDisplay};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

lazy_static! {
    static ref X_THREADS_INIT: () = {
        unsafe {
            XInitThreads();
        }
    };
}

/// A connection to the X11 display server.
#[derive(Clone)]
pub struct Connection {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
}

unsafe impl Send for Connection {}

pub(crate) struct NativeConnectionWrapper {
    display: *mut Display,
    pub(crate) egl_display: EGLDisplay,
    pub(crate) display_is_owned: bool,
}

/// Wrapper for an X11 `Display`.
#[derive(Clone)]
pub struct NativeConnection(pub *mut Display);

impl Drop for NativeConnectionWrapper {
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
            *X_THREADS_INIT;

            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(Error::ConnectionFailed);
            }

            let egl_display = create_egl_display(display);

            Ok(Connection {
                native_connection: Arc::new(NativeConnectionWrapper {
                    display,
                    egl_display,
                    display_is_owned: true,
                }),
            })
        }
    }

    /// Wraps an existing X11 `Display` in a `Connection`.
    ///
    /// Important: Before calling this function, X11 must have be initialized in a thread-safe
    /// manner by using `XInitThreads()`. Otherwise, it will not be safe to use `surfman` from
    /// multiple threads.
    ///
    /// The display is not retained, as there is no way to do that in the X11 API. Therefore, it is
    /// the caller's responsibility to ensure that the display connection is not closed before this
    /// `Connection` object is disposed of.
    #[inline]
    pub unsafe fn from_native_connection(native_connection: NativeConnection)
                                         -> Result<Connection, Error> {
        let egl_display = create_egl_display(native_connection.0);
        Ok(Connection {
            native_connection: Arc::new(NativeConnectionWrapper {
                display: native_connection.0,
                egl_display,
                display_is_owned: false,
            }),
        })
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection(self.native_connection.display)
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
    pub unsafe fn create_device_from_native_device(&self, native_device: NativeDevice)
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

impl NativeConnectionWrapper {
    #[inline]
    pub(crate) fn lock_display(&self) -> DisplayGuard {
        unsafe {
            let display = self.display;
            XLockDisplay(display);
            DisplayGuard { display, phantom: PhantomData }
        }
    }
}

pub(crate) struct DisplayGuard<'a> {
    display: *mut Display,
    phantom: PhantomData<&'a ()>,
}

impl<'a> Drop for DisplayGuard<'a> {
    fn drop(&mut self) {
        unsafe {
            XUnlockDisplay(self.display);
        }
    }
}

impl<'a> DisplayGuard<'a> {
    #[inline]
    pub(crate) fn display(&self) -> *mut Display {
        self.display
    }
}

unsafe fn create_egl_display(display: *mut Display) -> EGLDisplay {
    EGL_FUNCTIONS.with(|egl| {
        let display_attributes = [egl::NONE as EGLAttrib];
        let egl_display = egl.GetPlatformDisplay(EGL_PLATFORM_X11_KHR,
                                                 display as *mut c_void,
                                                 display_attributes.as_ptr());

        let (mut egl_major_version, mut egl_minor_version) = (0, 0);
        let ok = egl.Initialize(egl_display, &mut egl_major_version, &mut egl_minor_version);
        assert_ne!(ok, egl::FALSE);

        egl_display
    })
}

