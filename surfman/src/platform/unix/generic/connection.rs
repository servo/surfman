// surfman/surfman/src/platform/unix/generic/connection.rs
//
//! Represents a connection to a display server.

use crate::Error;
use crate::egl::types::{EGLAttrib, EGLDisplay};
use crate::egl;
use crate::info::GLApi;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_PLATFORM_SURFACELESS_MESA;
use super::device::{Adapter, Device, NativeDevice};
use super::surface::NativeWidget;

use euclid::default::Size2D;

use std::os::raw::c_void;
use std::sync::Arc;

#[cfg(feature = "sm-winit")]
use winit::window::Window;

/// A no-op connection.
#[derive(Clone)]
pub struct Connection {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
}

/// Native connections.
#[derive(Clone)]
pub struct NativeConnection(Arc<NativeConnectionWrapper>);

/// Native connections.
pub struct NativeConnectionWrapper {
    pub(crate) egl_display: EGLDisplay,
}

unsafe impl Send for NativeConnectionWrapper {}
unsafe impl Sync for NativeConnectionWrapper {}

impl Connection {
    /// Opens a surfaceless Mesa display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            EGL_FUNCTIONS.with(|egl| {
                let egl_display_attributes = [egl::NONE as EGLAttrib];
                let egl_display = egl.GetPlatformDisplay(EGL_PLATFORM_SURFACELESS_MESA,
                                                         egl::DEFAULT_DISPLAY as *mut c_void,
                                                         egl_display_attributes.as_ptr());
                if egl_display == egl::NO_DISPLAY {
                    return Err(Error::ConnectionFailed);
                }

                let (mut egl_major_version, mut egl_minor_version) = (0, 0);
                let ok = egl.Initialize(egl_display,
                                        &mut egl_major_version,
                                        &mut egl_minor_version);
                if ok == egl::FALSE {
                    return Err(Error::ConnectionFailed);
                }

                let native_connection = NativeConnection(Arc::new(NativeConnectionWrapper {
                    egl_display,
                }));

                Connection::from_native_connection(native_connection)
            })
        }
    }

    /// An alias for `Connection::new()`, present for consistency with other backends.
    #[inline]
    pub unsafe fn from_native_connection(native_connection: NativeConnection)
                                         -> Result<Connection, Error> {
        Ok(Connection { native_connection: native_connection.0 })
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection(self.native_connection.clone())
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    /// 
    /// This is an alias for `Connection::create_hardware_adapter()`.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    /// 
    /// On the OSMesa backend, this returns a software adapter.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter::hardware())
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    /// 
    /// On the OSMesa backend, this returns a software adapter.
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

    /// An alias for `connection.create_device()` with the default adapter.
    #[inline]
    pub unsafe fn create_device_from_native_device(&self, _: NativeDevice)
                                                   -> Result<Device, Error> {
        Device::new(self, &self.create_adapter()?) 
    }

    /// Opens the display connection corresponding to the given `winit` window.
    #[inline]
    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Err(Error::IncompatibleNativeWidget)
    }

    /// Creates a native widget type from the given `winit` window.
    /// 
    /// This type can be later used to create surfaces that render to the window.
    #[inline]
    #[cfg(feature = "sm-winit")]
    pub fn create_native_widget_from_winit_window(&self, _: &Window)
                                                  -> Result<NativeWidget, Error> {
        Err(Error::IncompatibleNativeWidget)
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(&self, _raw: *mut c_void, _size: Size2D<i32>) -> NativeWidget {
        NativeWidget
    }

    /// Create a native widget type from the given `raw_window_handle::RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle")]
    #[inline]
    pub fn create_native_widget_from_rwh(&self, _: raw_window_handle::RawWindowHandle)
                                                  -> Result<NativeWidget, Error> {
        Err(Error::IncompatibleNativeWidget)
    }
}

