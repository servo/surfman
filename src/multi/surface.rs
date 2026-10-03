//! A surface abstraction that allows the choice of backends dynamically.

use super::device::Device;
use crate::device::Device as DeviceInterface;
use crate::{Context, Error, Surface, SurfaceAccess, SurfaceInfo, SurfaceTexture, SurfaceType};
use euclid::default::Size2D;
use glow::Texture;

impl<Def, Alt> Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
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
        match self {
            Device::Default(device) => device.create_surface(context, surface_access, surface_type),
            Device::Alternate(device) => {
                device.create_surface(context, surface_access, surface_type)
            }
        }
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
        match self {
            Device::Default(device) => device.create_surface_texture(context, surface),
            Device::Alternate(device) => device.create_surface_texture(context, surface),
        }
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
        match self {
            Device::Default(device) => device.destroy_surface(context, surface),
            Device::Alternate(device) => device.destroy_surface(context, surface),
        }
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
        match self {
            Device::Default(device) => device.destroy_surface_texture(context, surface_texture),
            Device::Alternate(device) => device.destroy_surface_texture(context, surface_texture),
        }
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.present_surface(context, surface),
            Device::Alternate(device) => device.present_surface(context, surface),
        }
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.resize_surface(context, surface, size),
            Device::Alternate(device) => device.resize_surface(context, surface, size),
        }
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> u32 {
        match *self {
            Device::Default(ref device) => device.surface_gl_texture_target(),
            Device::Alternate(ref device) => device.surface_gl_texture_target(),
        }
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        match self {
            Device::Default(device) => device.surface_info(surface),
            Device::Alternate(device) => device.surface_info(surface),
        }
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> Option<Texture> {
        match self {
            Device::Default(device) => device.surface_texture_object(surface_texture),
            Device::Alternate(device) => device.surface_texture_object(surface_texture),
        }
    }
}
