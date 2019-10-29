// surfman/surfman/src/platform/unix/wayland/connection.rs
//
//! A wrapper for Wayland connections (displays).
//! 
//! TODO(pcwalton): Looks like we'll need a `Connection::from_winit_window()`.

use crate::Error;
use crate::egl::types::EGLDisplay;
use crate::egl;
use crate::platform::generic::egl::ffi::EGL_FUNCTIONS;

use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{WAYLAND_CLIENT_HANDLE, wl_display};

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

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection, Error> {
        unsafe {
            let wayland_display = match window.get_wayland_display() {
                Some(wayland_display) => wayland_display as *mut wl_display,
                None => ptr::null_mut(),
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

