// surfman/surfman/src/platform/unix/wayland/connection.rs
//
//! A wrapper for Wayland connections (displays).
//! 
//! TODO(pcwalton): Looks like we'll need a `Connection::from_winit_window()`.

use crate::Error;

use std::ffi::CString;
use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{wl_display, wl_display_connect, wl_display_destroy};

#[derive(Clone, Debug)]
pub struct Connection {
    pub(crate) native_connection: Box<dyn NativeConnection>,
}

pub(crate) trait NativeConnection: Clone {
    fn wayland_display(&self) -> *mut wl_display;
    unsafe fn destroy(&mut self);
}

impl Connection {
    /// Connects to the Wayland server
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            let wayland_display = wl_display_connect(ptr::null());
            if wayland_display.is_null() {
                return Err(Error::NoConnectionFound)
            };
            let wayland_display = Box::new(SharedConnection {
                wayland_display: Arc::new(WaylandDisplay(wayland_display)),
            };
            Ok(Connection { wayland_display })
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
}

struct WaylandDisplay(*mut wl_display);

impl Drop for WaylandDisplay {
    fn drop(&mut self) {
        unsafe {
            wl_display_destroy(self.0);
        }
    }
}
