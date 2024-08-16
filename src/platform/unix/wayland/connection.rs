// surfman/surfman/src/platform/unix/wayland/connection.rs
//
//! A wrapper for Wayland connections (displays).

use super::device::{Adapter, Device, NativeDevice};
use super::surface::NativeWidget;
use crate::egl;
use crate::egl::types::{EGLAttrib, EGLDisplay};
use crate::info::GLApi;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_PLATFORM_WAYLAND_KHR;
use crate::Error;

use euclid::default::Size2D;
use std::os::raw::c_void;
use std::ptr;
use std::sync::Arc;
use wayland_sys::client::{wl_display, wl_proxy, WAYLAND_CLIENT_HANDLE};

/// A connection to the Wayland server.
#[derive(Clone)]
pub struct Connection {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
}

pub(crate) struct NativeConnectionWrapper {
    pub(crate) egl_display: EGLDisplay,
    wayland_display: Option<*mut wl_display>,
}

/// An EGL display wrapping a Wayland display.
pub struct NativeConnection(pub EGLDisplay);

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

    /// Wraps an existing EGL display in a `Connection`.
    ///
    /// The display is not retained, as there is no way to do this in the EGL API. Therefore, it is
    /// the caller's responsibility to ensure that the EGL display remains alive as long as the
    /// connection is.
    pub unsafe fn from_native_connection(
        native_connection: NativeConnection,
    ) -> Result<Connection, Error> {
        Connection::from_egl_display(native_connection.0, None)
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection(self.native_connection.egl_display)
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        if std::env::var("SURFMAN_FORCE_GLES").is_ok() {
            GLApi::GLES
        } else {
            GLApi::GL
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

    /// Opens the hardware device corresponding to the adapter wrapped in the given native
    /// device.
    ///
    /// This is present for compatibility with other backends.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        native_device: NativeDevice,
    ) -> Result<Device, Error> {
        Device::new(self, &native_device.adapter)
    }

    unsafe fn from_wayland_display(
        wayland_display: *mut wl_display,
        is_owned: bool,
    ) -> Result<Connection, Error> {
        if wayland_display.is_null() {
            return Err(Error::ConnectionFailed);
        }

        EGL_FUNCTIONS.with(|egl| {
            let display_attributes = [egl::NONE as EGLAttrib];
            let egl_display = egl.GetPlatformDisplay(
                EGL_PLATFORM_WAYLAND_KHR,
                wayland_display as *mut c_void,
                display_attributes.as_ptr(),
            );
            if egl_display == egl::NO_DISPLAY {
                return Err(Error::DeviceOpenFailed);
            }

            let (mut egl_major_version, mut egl_minor_version) = (0, 0);
            let ok = egl.Initialize(egl_display, &mut egl_major_version, &mut egl_minor_version);
            assert_ne!(ok, egl::FALSE);

            let owned_display = if is_owned {
                Some(wayland_display)
            } else {
                None
            };
            Connection::from_egl_display(egl_display, owned_display)
        })
    }

    fn from_egl_display(
        egl_display: EGLDisplay,
        wayland_display: Option<*mut wl_display>,
    ) -> Result<Connection, Error> {
        Ok(Connection {
            native_connection: Arc::new(NativeConnectionWrapper {
                egl_display,
                wayland_display,
            }),
        })
    }

    /// Opens the display connection corresponding to the given `RawDisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn from_raw_display_handle(
        raw_handle: rwh_05::RawDisplayHandle,
    ) -> Result<Connection, Error> {
        use rwh_05::RawDisplayHandle::Wayland;
        use rwh_05::WaylandDisplayHandle;
        unsafe {
            let wayland_display = match raw_handle {
                Wayland(WaylandDisplayHandle { display, .. }) => display as *mut wl_display,
                _ => return Err(Error::IncompatibleRawDisplayHandle),
            };

            Connection::from_wayland_display(wayland_display, false)
        }
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn from_display_handle(handle: rwh_06::DisplayHandle) -> Result<Connection, Error> {
        use rwh_06::RawDisplayHandle::Wayland;
        use rwh_06::WaylandDisplayHandle;
        unsafe {
            let wayland_display = match handle.as_raw() {
                Wayland(WaylandDisplayHandle { display, .. }) => {
                    display.as_ptr() as *mut wl_display
                }
                _ => return Err(Error::IncompatibleRawDisplayHandle),
            };

            Connection::from_wayland_display(wayland_display, false)
        }
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        size: Size2D<i32>,
    ) -> NativeWidget {
        NativeWidget {
            wayland_surface: raw as *mut wl_proxy,
            size,
        }
    }

    /// Creates a native widget type from the given `RawWindowHandle`
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn create_native_widget_from_raw_window_handle(
        &self,
        raw_handle: rwh_05::RawWindowHandle,
        window_size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use rwh_05::RawWindowHandle::Wayland;

        let wayland_surface = match raw_handle {
            Wayland(handle) => handle.surface as *mut wl_proxy,
            _ => return Err(Error::IncompatibleNativeWidget),
        };

        Ok(NativeWidget {
            wayland_surface,
            size: window_size,
        })
    }

    /// Creates a native widget type from the given `WindowHandle`
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: rwh_06::WindowHandle,
        window_size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use rwh_06::RawWindowHandle::Wayland;

        let wayland_surface = match handle.as_raw() {
            Wayland(handle) => handle.surface.as_ptr() as *mut wl_proxy,
            _ => return Err(Error::IncompatibleNativeWidget),
        };

        Ok(NativeWidget {
            wayland_surface,
            size: window_size,
        })
    }
}

impl Drop for NativeConnectionWrapper {
    fn drop(&mut self) {
        unsafe {
            if let Some(wayland_display) = self.wayland_display {
                (WAYLAND_CLIENT_HANDLE.wl_display_disconnect)(wayland_display);
            }
        }
    }
}

impl NativeConnection {
    /// Returns the current native connection, if applicable.
    #[inline]
    pub fn current() -> Result<NativeConnection, Error> {
        unsafe {
            EGL_FUNCTIONS.with(|egl| {
                let display = egl.GetCurrentDisplay();
                if display != egl::NO_DISPLAY {
                    Ok(NativeConnection(display))
                } else {
                    Err(Error::NoCurrentConnection)
                }
            })
        }
    }
}
