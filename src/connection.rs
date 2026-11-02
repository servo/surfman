//! The abstract interface that all connections conform to.

use crate::adapter::AdapterPreferences;
use crate::macros::{dispatch, enum_conversion};
use crate::{Adapter, Device, Error, GLApi};
use raw_window_handle::DisplayHandle;

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

/// The backends a new connection tries, most preferred first.
const BACKENDS: &[fn() -> Result<Connection, Error>] = &[
    #[cfg(angle_default)]
    || AngleConnection::new().map(Connection::Angle),
    #[cfg(all(windows_platform, not(angle_default)))]
    || WglConnection::new().map(Connection::Wgl),
    #[cfg(macos_platform)]
    || CglConnection::new().map(Connection::Cgl),
    #[cfg(any(android_platform, ohos_platform))]
    || HardwareBufferConnection::new().map(Connection::HardwareBuffer),
    #[cfg(wayland_platform)]
    || WaylandConnection::new().map(Connection::Wayland),
    #[cfg(all(x11_platform, not(wayland_default)))]
    || X11Connection::new().map(Connection::X11),
    #[cfg(all(free_unix, not(wayland_default)))]
    || SurfacelessMesaConnection::new().map(Connection::SurfacelessMesa),
];

type DisplayHandleBackend = fn(DisplayHandle) -> Result<Connection, Error>;

/// The backends a connection to a display handle tries, in the same order as [`BACKENDS`].
const DISPLAY_HANDLE_BACKENDS: &[DisplayHandleBackend] = &[
    #[cfg(angle_default)]
    |handle| AngleConnection::from_display_handle(handle).map(Connection::Angle),
    #[cfg(all(windows_platform, not(angle_default)))]
    |handle| WglConnection::from_display_handle(handle).map(Connection::Wgl),
    #[cfg(macos_platform)]
    |handle| CglConnection::from_display_handle(handle).map(Connection::Cgl),
    #[cfg(any(android_platform, ohos_platform))]
    |handle| HardwareBufferConnection::from_display_handle(handle).map(Connection::HardwareBuffer),
    #[cfg(wayland_platform)]
    |handle| WaylandConnection::from_display_handle(handle).map(Connection::Wayland),
    #[cfg(all(x11_platform, not(wayland_default)))]
    |handle| X11Connection::from_display_handle(handle).map(Connection::X11),
    #[cfg(all(free_unix, not(wayland_default)))]
    |handle| {
        SurfacelessMesaConnection::from_display_handle(handle).map(Connection::SurfacelessMesa)
    },
];

impl Connection {
    /// Connects to the default display with the default backend of the platform. Builds with X11
    /// that don't default to Wayland try Wayland, then X11, then surfaceless Mesa.
    pub fn new() -> Result<Connection, Error> {
        let mut error = Error::ConnectionFailed;
        for backend in BACKENDS {
            match backend() {
                Ok(connection) => return Ok(connection),
                Err(backend_error) => error = backend_error,
            }
        }
        Err(error)
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`, with the backends
    /// [`Connection::new`] tries, in the same order.
    pub fn from_display_handle(handle: DisplayHandle) -> Result<Connection, Error> {
        let mut error = Error::IncompatibleRawDisplayHandle;
        for backend in DISPLAY_HANDLE_BACKENDS {
            match backend(handle) {
                Ok(connection) => return Ok(connection),
                Err(backend_error) => error = backend_error,
            }
        }
        Err(error)
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
    use crate::{ContextAttributeFlags, ContextAttributes, GLVersion};

    #[cfg_attr(not(feature = "sm-test"), test)]
    fn test_default_connection_creates_contexts() {
        let connection = Connection::new().unwrap();
        let adapter = connection.create_adapter(Default::default()).unwrap();
        let device = connection.create_device(&adapter).unwrap();
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
