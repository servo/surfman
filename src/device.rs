//! A platform-independent representation of a display device.

use crate::macros::{dispatch, enum_conversion};
use crate::{
    Adapter, Context, ContextAttributes, ContextDescriptor, ContextID, Error, GLApi, Surface,
    SurfaceAccess, SurfaceInfo, SurfaceTexture, SurfaceType,
};
use euclid::default::Size2D;
use glow::Texture;

use std::os::raw::c_void;

#[cfg(all(windows_platform, feature = "sm-angle"))]
use crate::angle::device::AngleDevice;
#[cfg(macos_platform)]
use crate::cgl::device::CglDevice;
#[cfg(any(android_platform, ohos_platform))]
use crate::hardware_buffer::device::HardwareBufferDevice;
#[cfg(free_unix)]
use crate::mesa_surfaceless::device::SurfacelessMesaDevice;
#[cfg(wayland_platform)]
use crate::wayland::device::WaylandDevice;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::device::WglDevice;
#[cfg(x11_platform)]
use crate::x11::device::X11Device;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub enum Device {
    /// An ANGLE device for Windows systems.
    #[cfg(all(windows_platform, feature = "sm-angle"))]
    Angle(AngleDevice),
    /// A device for Apple systems.
    #[cfg(macos_platform)]
    Cgl(CglDevice),
    /// A hardware buffer device for OHOS and Android systems.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(HardwareBufferDevice),
    /// A surfaceless Mesa device for X11 / Wayland systems.
    #[cfg(free_unix)]
    SurfacelessMesa(SurfacelessMesaDevice),
    /// A device for Wayland systems.
    #[cfg(wayland_platform)]
    Wayland(WaylandDevice),
    /// A WGL device for Windows systems.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglDevice),
    /// A device for X11 systems.
    #[cfg(x11_platform)]
    X11(X11Device),
}

#[cfg(all(windows_platform, feature = "sm-angle"))]
enum_conversion!(Device, Angle, AngleDevice, angle, IncompatibleDevice);
#[cfg(macos_platform)]
enum_conversion!(Device, Cgl, CglDevice, cgl, IncompatibleDevice);
#[cfg(any(android_platform, ohos_platform))]
enum_conversion!(
    Device,
    HardwareBuffer,
    HardwareBufferDevice,
    hardware_buffer,
    IncompatibleDevice
);
#[cfg(free_unix)]
enum_conversion!(
    Device,
    SurfacelessMesa,
    SurfacelessMesaDevice,
    surfaceless_mesa,
    IncompatibleDevice
);
#[cfg(wayland_platform)]
enum_conversion!(Device, Wayland, WaylandDevice, wayland, IncompatibleDevice);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(Device, Wgl, WglDevice, wgl, IncompatibleDevice);
#[cfg(x11_platform)]
enum_conversion!(Device, X11, X11Device, x11, IncompatibleDevice);

impl Device {
    /// Returns the adapter that this device was created with.
    pub fn adapter(&self) -> Adapter {
        dispatch!(Device, self, device => device.adapter())
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    pub fn gl_api(&self) -> GLApi {
        dispatch!(Device, self, device => device.gl_api())
    }

    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        dispatch!(Device, self, device => device.create_context_descriptor(attributes))
    }

    /// Creates a new OpenGL context and makes it current.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        dispatch!(Device, self, device => device.create_context(descriptor, share_with))
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        dispatch!(Device, self, device => device.destroy_context(context))
    }

    /// Returns the descriptor that this context was created with.
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        dispatch!(Device, self, device => device.context_descriptor(context))
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        dispatch!(Device, self, device => device.make_context_current(context))
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        dispatch!(Device, self, device => device.make_no_context_current())
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        dispatch!(Device, self, device => device.context_descriptor_attributes(context_descriptor))
    }

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    pub fn get_proc_address(&self, context: &Context, symbol_name: &str) -> *const c_void {
        dispatch!(Device, self, device => device.get_proc_address(context, symbol_name))
    }

    /// Attaches a surface to a context for rendering.
    ///
    /// This function takes ownership of the surface. The surface must have been created with this
    /// context, or an `IncompatibleSurface` error is returned.
    ///
    /// If this function is called with a surface already bound, a `SurfaceAlreadyBound` error is
    /// returned. To avoid this error, first unbind the existing surface with
    /// `unbind_surface_from_context`.
    ///
    /// If an error is returned, the surface is returned alongside it.
    pub fn bind_surface_to_context(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<(), (Error, Surface)> {
        dispatch!(Device, self, device => device.bind_surface_to_context(context, surface))
    }

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context,
    ) -> Result<Option<Surface>, Error> {
        dispatch!(Device, self, device => device.unbind_surface_from_context(context))
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    pub fn context_id(&self, context: &Context) -> ContextID {
        dispatch!(Device, self, device => device.context_id(context))
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        dispatch!(Device, self, device => device.context_surface_info(context))
    }

    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &self,
        context: &Context,
        surface_access: SurfaceAccess,
        surface_type: SurfaceType<'_>,
    ) -> Result<Surface, Error> {
        dispatch!(Device, self, device => device.create_surface(context, surface_access, surface_type))
    }

    /// Creates a surface texture from an existing generic surface for use with the given context.
    ///
    /// The surface texture is local to the supplied context and takes ownership of the surface.
    /// Destroying the surface texture allows you to retrieve the surface again.
    ///
    /// *The supplied context does not have to be the same context that the surface is associated
    /// with.* This allows you to render to a surface in one context and sample from that surface
    /// in another context.
    ///
    /// Calling this method on a widget surface returns a `WidgetAttached` error.
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        dispatch!(Device, self, device => device.create_surface_texture(context, surface))
    }

    /// Destroys a surface.
    ///
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(
        &self,
        context: &mut Context,
        surface: &mut Surface,
    ) -> Result<(), Error> {
        dispatch!(Device, self, device => device.destroy_surface(context, surface))
    }

    /// Destroys a surface texture and returns the underlying surface.
    ///
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    ///
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context,
        surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        dispatch!(Device, self, device => device.destroy_surface_texture(context, surface_texture))
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    pub fn surface_gl_texture_target(&self) -> u32 {
        dispatch!(Device, self, device => device.surface_gl_texture_target())
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        dispatch!(Device, self, device => device.present_bound_surface(context))
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        dispatch!(Device, self, device => device.present_surface(context, surface))
    }

    /// If the currently bound surface is a widget surface, resize it,
    pub fn resize_bound_surface(
        &self,
        context: &mut Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        dispatch!(Device, self, device => device.resize_bound_surface(context, size))
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        dispatch!(Device, self, device => device.resize_surface(context, surface, size))
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        dispatch!(Device, self, device => device.surface_info(surface))
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> Option<Texture> {
        dispatch!(Device, self, device => device.surface_texture_object(surface_texture))
    }
}
