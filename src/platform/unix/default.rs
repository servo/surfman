// surfman/src/platform/unix/default.rs
//
//! The default backend for Unix, which dynamically switches between Wayland, X11 and surfaceless.

/// Wayland or X11 display server connections.
pub mod connection {
    use crate::platform::generic::multi::connection::Connection as MultiConnection;
    use crate::platform::generic::multi::connection::NativeConnection as MultiNativeConnection;
    use crate::platform::generic::multi::device::Device as MultiDevice;
    use crate::platform::unix::generic::device::Device as SWDevice;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    type HWDevice = MultiDevice<WaylandDevice, X11Device>;

    /// Either a Wayland or an X11 display server connection.
    pub type Connection = MultiConnection<HWDevice, SWDevice>;

    /// Either a Wayland or an X11 native connection
    pub type NativeConnection = MultiNativeConnection<HWDevice, SWDevice>;
}

/// OpenGL rendering contexts.
pub mod context {
    use crate::platform::generic::multi::context::Context as MultiContext;
    use crate::platform::generic::multi::context::ContextDescriptor as MultiContextDescriptor;
    use crate::platform::generic::multi::context::NativeContext as MultiNativeContext;
    use crate::platform::generic::multi::device::Device as MultiDevice;
    use crate::platform::unix::generic::device::Device as SWDevice;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    type HWDevice = MultiDevice<WaylandDevice, X11Device>;

    /// Represents an OpenGL rendering context.
    ///
    /// A context allows you to issue rendering commands to a surface. When initially created, a
    /// context has no attached surface, so rendering commands will fail or be ignored. Typically,
    /// you attach a surface to the context before rendering.
    ///
    /// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in
    /// any way other than rendering to it (e.g. presenting it to a window, which causes a buffer
    /// swap), it must first be detached from its context. Each surface is associated with a single
    /// context upon creation and may not be rendered to from any other context. However, you can
    /// wrap a surface in a surface texture, which allows the surface to be read from another
    /// context.
    ///
    /// OpenGL objects may not be shared across contexts directly, but surface textures effectively
    /// allow for sharing of texture data. Contexts are local to a single thread and device.
    ///
    /// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
    pub type Context = MultiContext<HWDevice, SWDevice>;

    /// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
    ///
    /// These are local to a device.
    pub type ContextDescriptor = MultiContextDescriptor<HWDevice, SWDevice>;

    /// Either a Wayland or an X11 native context
    pub type NativeContext = MultiNativeContext<HWDevice, SWDevice>;
}

/// Thread-local handles to devices.
pub mod device {
    use crate::platform::generic::multi::device::Adapter as MultiAdapter;
    use crate::platform::generic::multi::device::NativeDevice as MultiNativeDevice;
    use crate::platform::unix::generic::device::Device as SWDevice;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;

    use crate::platform::generic::multi::device::Device as MultiDevice;
    type HWDevice = MultiDevice<WaylandDevice, X11Device>;

    /// Represents a hardware display adapter that can be used for rendering (including the CPU).
    ///
    /// Adapters can be sent between threads. To render with an adapter, open a thread-local
    /// `Device`.
    pub type Adapter = MultiAdapter<HWDevice, SWDevice>;

    /// A thread-local handle to a device.
    ///
    /// Devices contain most of the relevant surface management methods.
    pub type Device = MultiDevice<HWDevice, SWDevice>;

    /// Either a Wayland or an X11 native device
    pub type NativeDevice = MultiNativeDevice<HWDevice, SWDevice>;
}

/// Hardware buffers of pixels.
pub mod surface {
    use crate::platform::generic::multi::device::Device as MultiDevice;
    use crate::platform::generic::multi::surface::NativeWidget as MultiNativeWidget;
    use crate::platform::generic::multi::surface::Surface as MultiSurface;
    use crate::platform::generic::multi::surface::SurfaceTexture as MultiSurfaceTexture;
    use crate::platform::unix::generic::device::Device as SWDevice;
    use crate::platform::unix::wayland::device::Device as WaylandDevice;
    use crate::platform::unix::x11::device::Device as X11Device;
    type HWDevice = MultiDevice<WaylandDevice, X11Device>;

    /// A wrapper for a Wayland surface or an X11 `Window`, as appropriate.
    pub type NativeWidget = MultiNativeWidget<HWDevice, SWDevice>;

    /// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and
    /// either displayed in a native widget or bound to a texture for reading.
    ///
    /// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound
    /// to a texture but cannot be displayed in a widget (without using other APIs such as Core
    /// Animation, DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be
    /// displayed in a widget but not bound to a texture.
    ///
    /// Surfaces are specific to a given context and cannot be rendered to from any context other
    /// than the one they were created with. However, they can be *read* from any context on any
    /// thread (as long as that context shares the same adapter and connection), by wrapping them
    /// in a `SurfaceTexture`.
    ///
    /// Depending on the platform, each surface may be internally double-buffered.
    ///
    /// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
    pub type Surface = MultiSurface<HWDevice, SWDevice>;

    /// Represents an OpenGL texture that wraps a surface.
    ///
    /// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior
    /// to write to such a texture (e.g. by binding it to a framebuffer and rendering to that
    /// framebuffer).
    ///
    /// Surface textures are local to a context, but that context does not have to be the same
    /// context as that associated with the underlying surface. The texture must be destroyed with
    /// the `destroy_surface_texture()` method, or a panic will occur.
    pub type SurfaceTexture = MultiSurfaceTexture<HWDevice, SWDevice>;

    // FIXME(pcwalton): Revamp how this works.
    #[doc(hidden)]
    pub struct SurfaceDataGuard {}
}
