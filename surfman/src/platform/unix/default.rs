// surfman/src/platform/unix/default.rs
//
//! The default backend for Unix, which dynamically switches between Wayland and X11.

pub mod adapter {
    use crate::platform::generic::multi::adapter::Adapter as MultiAdapter;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    pub type Adapter = MultiAdapter<WaylandDevice, X11Device>;
}

pub mod connection {
    use crate::platform::generic::multi::connection::Connection as MultiConnection;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    pub type Connection = MultiConnection<WaylandDevice, X11Device>;
}

pub mod context {
    use crate::platform::generic::multi::context::Context as MultiContext;
    use crate::platform::generic::multi::context::ContextDescriptor as MultiContextDescriptor;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    pub type Context = MultiContext<WaylandDevice, X11Device>;
    pub type ContextDescriptor = MultiContextDescriptor<WaylandDevice, X11Device>;
}

pub mod device {
    use crate::platform::generic::multi::device::Device as MultiDevice;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    pub type Device = MultiDevice<WaylandDevice, X11Device>;
}

pub mod surface {
    use crate::platform::generic::multi::surface::NativeWidget as MultiNativeWidget;
    use crate::platform::generic::multi::surface::Surface as MultiSurface;
    use crate::platform::generic::multi::surface::SurfaceTexture as MultiSurfaceTexture;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    pub type NativeWidget = MultiNativeWidget<WaylandDevice, X11Device>;
    pub type Surface = MultiSurface<WaylandDevice, X11Device>;
    pub type SurfaceTexture = MultiSurfaceTexture<WaylandDevice, X11Device>;

    // FIXME(pcwalton): Revamp how this works.
    pub struct SurfaceDataGuard {}
}

