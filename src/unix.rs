// surfman/src/platform/unix/default.rs
//
//! The default backend for Unix, which dynamically switches between Wayland, X11 and surfaceless.

/// Wayland or X11 display server connections.
pub mod connection {
    use crate::mesa_surfaceless::connection::SurfacelessMesaConnection as SWConnection;
    use crate::multi::connection::Connection as MultiConnection;
    use crate::wayland::connection::WaylandConnection;
    use crate::x11::connection::X11Connection;
    type HWConnection = MultiConnection<WaylandConnection, X11Connection>;

    /// Either a Wayland or an X11 display server connection.
    pub type Connection = MultiConnection<HWConnection, SWConnection>;
}

/// Hardware buffers of pixels.
pub mod surface {
    // FIXME(pcwalton): Revamp how this works.
    #[doc(hidden)]
    pub struct SurfaceDataGuard {}
}
