// surfman/surfman/src/platform/unix/wayland/connection.rs
//
//! A wrapper for Wayland connections (displays).

use crate::Error;
use crate::egl::types::EGLDisplay;
use crate::egl;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use super::device::{Adapter, Device, NativeDevice};
use super::surface::NativeWidget;

use euclid::default::Size2D;
use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{WAYLAND_CLIENT_HANDLE, wl_display, wl_proxy};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

/// A connection to the Wayland server.
#[derive(Clone)]
pub struct Connection {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
}

pub(crate) struct NativeConnectionWrapper {
    pub(crate) wayland_display: *mut wl_display,
    pub(crate) egl_display: EGLDisplay,
    pub(crate) is_owned: bool,
}

/// Wrapper for a Wayland display.
pub struct NativeConnection(pub *mut wl_display);

unsafe impl Send for Connection {}

impl Connection {
    /// Connects to the default Wayland server.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            let wayland_display = (WAYLAND_CLIENT_HANDLE.wl_display_connect)(ptr::null());
            Connection::from_wayland_display(wayland_display, true)
        }
    }

    /// Wraps an existing Wayland display in a `Connection`.
    ///
    /// The display is not retained, as there is no way to do this in the Wayland API. Therefore,
    /// it is the caller's responsibility to ensure that the Wayland display remains alive as long
    /// as the connection is.
    pub unsafe fn from_native_connection(native_connection: NativeConnection)
                                         -> Result<Connection, Error> {
        Connection::from_wayland_display(native_connection.0, false)
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection(self.native_connection.wayland_display)
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

    unsafe fn from_wayland_display(wayland_display: *mut wl_display, is_owned: bool)
                                   -> Result<Connection, Error> {
        if wayland_display.is_null() {
            return Err(Error::ConnectionFailed);
        }

        EGL_FUNCTIONS.with(|egl| {
            let egl_display = egl.GetDisplay(wayland_display as *const _);
            if egl_display == egl::NO_DISPLAY {
                return Err(Error::DeviceOpenFailed);
            }

            let (mut egl_major_version, mut egl_minor_version) = (0, 0);
            let ok = egl.Initialize(egl_display, &mut egl_major_version, &mut egl_minor_version);
            assert_ne!(ok, egl::FALSE);

            Ok(Connection {
                native_connection: Arc::new(NativeConnectionWrapper {
                    wayland_display,
                    egl_display,
                    is_owned,
                })
            })
        })
    }

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        unsafe {
            let wayland_display = match window.get_wayland_display() {
                Some(wayland_display) => wayland_display as *mut wl_display,
                None => return Err(Error::IncompatibleWinitWindow),
            };
            Connection::from_wayland_display(wayland_display, false)
        }
    }

    /// Creates a native widget type from the given `winit` window.
    /// 
    /// This type can be later used to create surfaces that render to the window.
    #[cfg(feature = "sm-winit")]
    pub fn create_native_widget_from_winit_window(&self, window: &Window)
                                                  -> Result<NativeWidget, Error> {
        let wayland_surface = match window.get_wayland_surface() {
            Some(wayland_surface) => wayland_surface as *mut wl_proxy,
            None => return Err(Error::IncompatibleNativeWidget),
        };

        // The window's DPI factor is 1.0 when nothing has been rendered to it yet. So use the DPI
        // factor of the primary monitor instead, since that's where the window will presumably go
        // when actually displayed. (The user might move it somewhere else later, of course.)
        //
        // FIXME(pcwalton): Is it true that the window will go the primary monitor first?
        let hidpi_factor = window.get_primary_monitor().get_hidpi_factor();
        let window_size = window.get_inner_size().unwrap().to_physical(hidpi_factor);
        let window_size = Size2D::new(window_size.width as i32, window_size.height as i32);

        Ok(NativeWidget { wayland_surface, size: window_size })
    }
}

impl Drop for NativeConnectionWrapper {
    fn drop(&mut self) {
        unsafe {
            if self.is_owned {
                (WAYLAND_CLIENT_HANDLE.wl_display_disconnect)(self.wayland_display);
            }
        }
    }
}
