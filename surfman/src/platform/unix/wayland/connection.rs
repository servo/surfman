// surfman/surfman/src/platform/unix/wayland/connection.rs
//
//! A wrapper for Wayland connections (displays).
//! 
//! TODO(pcwalton): Looks like we'll need a `Connection::from_winit_window()`.

use crate::Error;

use std::ffi::CString;
use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{WAYLAND_CLIENT_HANDLE, wl_display};

pub struct Connection {
    pub(crate) native_connection: Box<dyn NativeConnection>,
}

pub(crate) trait NativeConnection {
    fn wayland_display(&self) -> *mut wl_display;
    fn retain(&self) -> Box<dyn NativeConnection>;
}

unsafe impl Send for Connection {}

impl Clone for Connection {
    fn clone(&self) -> Connection {
        Connection { native_connection: self.native_connection.retain() }
    }
}

impl Connection {
    /// Connects to the Wayland server
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            let wayland_display = (WAYLAND_CLIENT_HANDLE.wl_display_connect)(ptr::null());
            if wayland_display.is_null() {
                return Err(Error::ConnectionFailed);
            };
            let native_connection = Box::new(SharedConnection {
                wayland_display: Arc::new(WaylandDisplay(wayland_display)),
            });
            Ok(Connection { native_connection })
        }
    }
}

#[derive(Clone)]
struct SharedConnection {
    wayland_display: Arc<WaylandDisplay>,
}

impl NativeConnection for SharedConnection {
    fn wayland_display(&self) -> *mut wl_display {
        self.wayland_display.0
    }

    fn retain(&self) -> Box<dyn NativeConnection> {
        Box::new((*self).clone())
    }
}

struct WaylandDisplay(*mut wl_display);

impl Drop for WaylandDisplay {
    fn drop(&mut self) {
        unsafe {
            (WAYLAND_CLIENT_HANDLE.wl_display_disconnect)(self.0);
        }
    }
}

