//! A connection to the window server.
//!
//! It might seem like this should wrap an `EGLDisplay`, but it doesn't. Unfortunately, in the
//! ANGLE implementation `EGLDisplay` is not thread-safe, while `surfman` connections must be
//! thread-safe. So we need to use the DXGI/Direct3D concept of a connection instead. These are
//! implicit in the Win32 API, and as such this type is a no-op.

use super::adapter::AngleAdapter;
use super::device::{Device, NativeDevice, VendorPreference};
use crate::egl::types::EGLDisplay;
use crate::{Adapter, AdapterPreferences, Error, GLApi, PowerPreference, RenderingPreference};

use winapi::shared::minwindef::UINT;
use winapi::um::d3dcommon::{D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP};

const INTEL_PCI_ID: UINT = 0x8086;

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

    /// Returns an adapter on this system according to the provided preferences.
    #[inline]
    pub fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error> {
        let driver_type = match preferences.rendering {
            RenderingPreference::Hardware => D3D_DRIVER_TYPE_UNKNOWN,
            RenderingPreference::Software => D3D_DRIVER_TYPE_WARP,
        };
        let vendor_preference = match preferences.power {
            PowerPreference::HighPerformance => VendorPreference::Avoid(INTEL_PCI_ID),
            PowerPreference::LowPower => VendorPreference::Prefer(INTEL_PCI_ID),
        };
        AngleAdapter::new(driver_type, vendor_preference).map(Into::into)
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new(adapter.angle()?)
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

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    pub fn from_display_handle(_: raw_window_handle::DisplayHandle) -> Result<Connection, Error> {
        Connection::new()
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
