//! A wrapper for Wayland adapters (displays).
//! 
//! TODO(pcwalton): Looks like we'll need an `Adapter::from_winit_window()`.

use crate::Error;

use std::ffi::CString;
use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{wl_display, wl_display_connect, wl_display_destroy};

#[derive(Clone, Debug)]
pub struct Adapter {
    pub(crate) native_adapter: Box<dyn NativeAdapter>,
}

pub(crate) trait NativeAdapter: Clone {
    fn wayland_display(&self) -> *mut wl_display;
    unsafe fn destroy(&mut self);
}

impl Adapter {
    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn default() -> Result<Adapter, Error> {
        unsafe {
            let wayland_display = wl_display_connect(ptr::null());
            if wayland_display.is_null() {
                return Err(Error::NoAdapterFound)
            };
            let wayland_display = Box::new(SharedAdapter {
                wayland_display: Arc::new(WaylandDisplay(wayland_display)),
            };
            Ok(Adapter { wayland_display })
        }
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn hardware() -> Result<Adapter, Error> {
        Adapter::default()
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The Wayland backend has no software support, so this returns an error. You can use the
    /// universal backend to get a software adapter.
    ///
    /// TODO(pcwalton): If Mesa is in use, maybe we could use `llvmpipe` somehow?
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }
}

#[derive(Clone)]
struct SharedAdapter {
    wayland_display: Arc<WaylandDisplay>,
}

impl NativeAdapter for SharedAdapter {
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
