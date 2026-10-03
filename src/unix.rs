// surfman/src/platform/unix/default.rs
//
//! The default backend for Unix, which dynamically switches between Wayland, X11 and surfaceless.

/// Wayland or X11 display server connections.
pub mod connection {
    use crate::mesa_surfaceless::device::Device as SWDevice;
    use crate::multi::connection::Connection as MultiConnection;
    use crate::multi::device::Device as MultiDevice;
    use crate::wayland::device::Device as WaylandDevice;
    use crate::x11::device::Device as X11Device;
    type HWDevice = MultiDevice<WaylandDevice, X11Device>;

    /// Either a Wayland or an X11 display server connection.
    pub type Connection = MultiConnection<HWDevice, SWDevice>;
}

/// Thread-local handles to devices.
pub mod device {
    use crate::mesa_surfaceless::device::Device as SWDevice;
    use crate::wayland::device::Device as WaylandDevice;
    use crate::x11::device::Device as X11Device;

    use crate::multi::device::Device as MultiDevice;
    type HWDevice = MultiDevice<WaylandDevice, X11Device>;

    /// A thread-local handle to a device.
    ///
    /// Devices contain most of the relevant surface management methods.
    pub type Device = MultiDevice<HWDevice, SWDevice>;
}

/// Hardware buffers of pixels.
pub mod surface {
    // FIXME(pcwalton): Revamp how this works.
    #[doc(hidden)]
    pub struct SurfaceDataGuard {}
}
