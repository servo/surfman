//! A connection to the display server, which wraps a platform-specific backend connection.

use crate::adapter::AdapterPreferences;
use crate::macros::{dispatch, enum_conversion};
use crate::{Adapter, Device, Error, GLApi};
use raw_window_handle::DisplayHandle;
#[cfg(all(free_unix, not(wayland_default)))]
use raw_window_handle::RawDisplayHandle;

#[cfg(all(windows_platform, feature = "sm-angle"))]
use crate::angle::connection::AngleConnection;
#[cfg(macos_platform)]
use crate::cgl::connection::CglConnection;
#[cfg(any(android_platform, ohos_platform))]
use crate::hardware_buffer::connection::HardwareBufferConnection;
#[cfg(free_unix)]
use crate::mesa_surfaceless::connection::SurfacelessMesaConnection;
#[cfg(wayland_platform)]
use crate::wayland::connection::WaylandConnection;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::connection::WglConnection;
#[cfg(x11_platform)]
use crate::x11::connection::X11Connection;

/// A connection to the display server.
#[derive(Clone)]
pub enum Connection {
    /// An ANGLE connection for Windows systems.
    #[cfg(all(windows_platform, feature = "sm-angle"))]
    Angle(AngleConnection),
    /// A connection for Apple systems.
    #[cfg(macos_platform)]
    Cgl(CglConnection),
    /// A hardware buffer connection for OHOS and Android systems.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(HardwareBufferConnection),
    /// A surfaceless Mesa connection for X11 / Wayland systems.
    #[cfg(free_unix)]
    SurfacelessMesa(SurfacelessMesaConnection),
    /// A connection to a Wayland server.
    #[cfg(wayland_platform)]
    Wayland(WaylandConnection),
    /// A WGL connection for Windows systems.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglConnection),
    /// A connection to an X11 server.
    #[cfg(x11_platform)]
    X11(X11Connection),
}

#[cfg(all(windows_platform, feature = "sm-angle"))]
enum_conversion!(
    Connection,
    Angle,
    AngleConnection,
    angle,
    IncompatibleConnection
);
#[cfg(macos_platform)]
enum_conversion!(Connection, Cgl, CglConnection, cgl, IncompatibleConnection);
#[cfg(any(android_platform, ohos_platform))]
enum_conversion!(
    Connection,
    HardwareBuffer,
    HardwareBufferConnection,
    hardware_buffer,
    IncompatibleConnection
);
#[cfg(free_unix)]
enum_conversion!(
    Connection,
    SurfacelessMesa,
    SurfacelessMesaConnection,
    surfaceless_mesa,
    IncompatibleConnection
);
#[cfg(wayland_platform)]
enum_conversion!(
    Connection,
    Wayland,
    WaylandConnection,
    wayland,
    IncompatibleConnection
);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(Connection, Wgl, WglConnection, wgl, IncompatibleConnection);
#[cfg(x11_platform)]
enum_conversion!(Connection, X11, X11Connection, x11, IncompatibleConnection);

impl Connection {
    /// Connects to the default display with the default backend of the platform. On Linux and BSD,
    /// when building without `sm-wayland-default` and with `sm-x11` this will try Wayland, X11, and
    /// fall back to Surfaceless Mesa. Otherwise, it will only try Wayland.
    pub fn new() -> Result<Connection, Error> {
        #[cfg(angle_default)]
        return AngleConnection::new().map(Into::into);
        #[cfg(all(windows_platform, not(angle_default)))]
        return WglConnection::new().map(Into::into);
        #[cfg(macos_platform)]
        return CglConnection::new().map(Into::into);
        #[cfg(any(android_platform, ohos_platform))]
        return HardwareBufferConnection::new().map(Into::into);
        #[cfg(wayland_default)]
        return WaylandConnection::new().map(Into::into);
        #[cfg(all(free_unix, not(wayland_default)))]
        return WaylandConnection::new()
            .map(Into::into)
            .or_else(|_| X11Connection::new().map(Into::into))
            .or_else(|_| SurfacelessMesaConnection::new().map(Into::into));
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`. When more than one
    /// backend is supported, this will examine the type of the handle to return the correct
    /// [`Connection`] type.
    pub fn from_display_handle(handle: DisplayHandle) -> Result<Connection, Error> {
        #[cfg(angle_default)]
        return AngleConnection::from_display_handle(handle).map(Into::into);
        #[cfg(all(windows_platform, not(angle_default)))]
        return WglConnection::from_display_handle(handle).map(Into::into);
        #[cfg(macos_platform)]
        return CglConnection::from_display_handle(handle).map(Into::into);
        #[cfg(any(android_platform, ohos_platform))]
        return HardwareBufferConnection::from_display_handle(handle).map(Into::into);
        #[cfg(wayland_default)]
        return WaylandConnection::from_display_handle(handle).map(Into::into);
        #[cfg(all(free_unix, not(wayland_default)))]
        return match handle.as_raw() {
            RawDisplayHandle::Xlib(..) | RawDisplayHandle::Xcb(..) => {
                X11Connection::from_display_handle(handle).map(Into::into)
            }
            _ => WaylandConnection::from_display_handle(handle).map(Into::into),
        };
    }

    /// Returns the OpenGL API flavor that this connection supports (OpenGL or OpenGL ES).
    pub fn gl_api(&self) -> GLApi {
        dispatch!(Connection, self, connection => connection.gl_api())
    }

    /// Returns an adapter on this system according to the provided preferences.
    pub fn create_adapter(&self, preferences: AdapterPreferences) -> Result<Adapter, Error> {
        dispatch!(Connection, self, connection => connection.create_adapter(preferences))
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        dispatch!(Connection, self, connection => connection.create_device(adapter))
    }
}

#[cfg(test)]
mod tests {
    use super::Connection;
    use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion};

    #[test]
    fn test_default_connection_creates_contexts() {
        let connection = Connection::new().unwrap();
        let adapter = connection.create_adapter(Default::default()).unwrap();
        let device = match connection.create_device(&adapter) {
            Ok(device) => device,
            Err(Error::RequiredExtensionUnavailable) => {
                // Can't run this test on this hardware.
                return;
            }
            Err(err) => panic!("Failed to create device: {:?}", err),
        };
        let descriptor = device
            .create_context_descriptor(&ContextAttributes {
                version: GLVersion::new(3, 0),
                flags: ContextAttributeFlags::empty(),
            })
            .unwrap();
        let mut context = device.create_context(&descriptor, None).unwrap();
        device.make_context_current(&context).unwrap();
        device.destroy_context(&mut context).unwrap();
    }
}
