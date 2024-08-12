// surfman/surfman/src/platform/windows/angle/connection.rs
//
//! A connection to the window server.
//!
//! It might seem like this should wrap an `EGLDisplay`, but it doesn't. Unfortunately, in the
//! ANGLE implementation `EGLDisplay` is not thread-safe, while `surfman` connections must be
//! thread-safe. So we need to use the DXGI/Direct3D concept of a connection instead. These are
//! implicit in the Win32 API, and as such this type is a no-op.

use super::device::{Adapter, Device, NativeDevice, VendorPreference};
use super::surface::NativeWidget;
use crate::egl::types::{EGLDisplay, EGLNativeWindowType};
use crate::Error;
use crate::GLApi;

use euclid::default::Size2D;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP};

use std::os::raw::c_void;


const INTEL_PCI_ID: u32 = 0x8086;

/// A no-op connection.
///
/// It might seem like this should wrap an `EGLDisplay`, but it doesn't. Unfortunately, in the
/// ANGLE implementation `EGLDisplay` is not thread-safe, while `surfman` connections must be
/// thread-safe. So we need to use the DXGI/Direct3D concept of a connection instead. These are
/// implicit in the Win32 API, and as such this type is a no-op.
#[derive(Clone)]
pub struct Connection;

/// An empty placeholder for native connections.
///
/// It might seem like this should wrap an `EGLDisplay`, but it doesn't. Unfortunately, in the
/// ANGLE implementation `EGLDisplay` is not thread-safe, while `surfman` connections must be
/// thread-safe. So we need to use the DXGI/Direct3D concept of a connection instead. These are
/// implicit in the Win32 API, and as such this type is a no-op.
#[derive(Clone)]
pub struct NativeConnection;

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection)
    }

    /// An alias for `Connection::new()`, present for consistency with other backends.
    #[inline]
    pub unsafe fn from_native_connection(_: NativeConnection) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
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
        Adapter::new(
            D3D_DRIVER_TYPE_UNKNOWN,
            VendorPreference::Avoid(INTEL_PCI_ID),
        )
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Adapter::new(
            D3D_DRIVER_TYPE_UNKNOWN,
            VendorPreference::Prefer(INTEL_PCI_ID),
        )
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        Adapter::new(D3D_DRIVER_TYPE_WARP, VendorPreference::None)
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(adapter)
    }

    /// Wraps a `NativeDevice` in a `Device` and returns it.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        native_device: NativeDevice,
    ) -> Result<Device, Error> {
        Device::from_native_device(native_device)
    }

    /// Wraps an ANGLE `EGLDisplay`, along with the associated Direct3D device, in a `Device` and
    /// returns it.
    ///
    /// The underlying `EGLDisplay` is not retained, as there is no way to do this in the EGL API.
    /// Therefore, it is the caller's responsibility to keep it alive as long as this `Device`
    /// remains alive. This function does, however, call `AddRef` on the Direct3D device.
    #[inline]
    pub unsafe fn create_device_from_egl_display(
        &self,
        egl_display: EGLDisplay,
    ) -> Result<Device, Error> {
        Device::from_egl_display(egl_display)
    }

    /// Opens the display connection corresponding to the given `RawDisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn from_raw_display_handle(_: rwh_05::RawDisplayHandle) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn from_display_handle(_: rwh_06::DisplayHandle) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        _size: Size2D<i32>,
    ) -> NativeWidget {
        NativeWidget {
            egl_native_window: raw as EGLNativeWindowType,
        }
    }

    /// Create a native widget type from the given `RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    #[inline]
    pub fn create_native_widget_from_raw_window_handle(
        &self,
        handle: rwh_05::RawWindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        if let rwh_05::RawWindowHandle::Win32(handle) = handle {
            Ok(NativeWidget {
                egl_native_window: handle.hwnd as EGLNativeWindowType,
            })
        } else {
            Err(Error::IncompatibleNativeWidget)
        }
    }

    /// Create a native widget type from the given `WindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    #[inline]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: rwh_06::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        if let rwh_06::RawWindowHandle::Win32(handle) = handle.as_raw() {
            Ok(NativeWidget {
                egl_native_window: handle.hwnd.get() as EGLNativeWindowType,
            })
        } else {
            Err(Error::IncompatibleNativeWidget)
        }
    }
}

impl NativeConnection {
    /// Creates a native connection.
    ///
    /// This is a no-op method present for consistency with other backends.
    #[inline]
    pub fn new() -> NativeConnection {
        NativeConnection
    }
}
