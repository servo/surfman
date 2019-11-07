// surfman/surfman/src/platform/unix/wayland/connection.rs
//
//! A wrapper for Wayland connections (displays).

use crate::Error;
use crate::egl::types::EGLDisplay;
use crate::egl;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use super::adapter::Adapter;
use super::device::Device;
use super::surface::NativeWidget;

use euclid::default::Size2D;
use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{WAYLAND_CLIENT_HANDLE, wl_display, wl_proxy};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

pub struct Connection {
    pub(crate) native_connection: Box<dyn NativeConnection>,
}

pub(crate) trait NativeConnection {
    fn wayland_display(&self) -> *mut wl_display;
    fn egl_display(&self) -> EGLDisplay;
    fn retain(&self) -> Box<dyn NativeConnection>;
}

unsafe impl Send for Connection {}

impl Clone for Connection {
    fn clone(&self) -> Connection {
        Connection { native_connection: self.native_connection.retain() }
    }
}

impl Connection {
    /// Connects to the Wayland server.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            let wayland_display = (WAYLAND_CLIENT_HANDLE.wl_display_connect)(ptr::null());
            Connection::from_wayland_display(wayland_display)
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
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::Hardware)
    }

    /// Returns the "best" software adapter on this system.
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
        unsafe {
            let wayland_display = match window.get_wayland_display() {
                Some(wayland_display) => wayland_display as *mut wl_display,
                None => return Err(Error::IncompatibleWinitWindow),
            };
            Connection::from_wayland_display(wayland_display)
        }
    }

    unsafe fn from_wayland_display(wayland_display: *mut wl_display)
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

            let native_connection = Box::new(SharedConnection {
                display: Arc::new(Display { wayland: wayland_display, egl: egl_display }),
            });
            Ok(Connection { native_connection })
        })
    }

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

#[derive(Clone)]
struct SharedConnection {
    display: Arc<Display>,
}

impl NativeConnection for SharedConnection {
    fn wayland_display(&self) -> *mut wl_display {
        self.display.wayland
    }

    fn egl_display(&self) -> EGLDisplay {
        self.display.egl
    }

    fn retain(&self) -> Box<dyn NativeConnection> {
        Box::new((*self).clone())
    }
}

struct Display {
    wayland: *mut wl_display,
    egl: EGLDisplay,
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            (WAYLAND_CLIENT_HANDLE.wl_display_disconnect)(self.wayland);
        }
    }
}
