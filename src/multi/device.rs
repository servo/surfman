//! A device abstraction that allows the choice of backends dynamically.

use super::connection::Connection;
use crate::connection::Connection as ConnectionInterface;
use crate::context::ContextAttributes;
use crate::device::Device as DeviceInterface;
use crate::{
    Adapter, Context, ContextDescriptor, ContextID, Error, GLApi, Surface, SurfaceAccess,
    SurfaceInfo, SurfaceTexture, SurfaceType,
};
use euclid::default::Size2D;
use glow::Texture;

use std::os::raw::c_void;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub enum Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default device type.
    Default(Def),
    /// The alternate device type.
    Alternate(Alt),
}

impl<Def, Alt> Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: ConnectionInterface,
    Alt::Connection: ConnectionInterface,
{
    /// Returns the display server connection that this device was created with.
    pub fn connection(&self) -> Connection<Def, Alt> {
        match *self {
            Device::Default(ref device) => Connection::Default(device.connection()),
            Device::Alternate(ref device) => Connection::Alternate(device.connection()),
        }
    }

    /// Returns the adapter that this device was created with.
    pub fn adapter(&self) -> Adapter {
        match *self {
            Device::Default(ref device) => device.adapter(),
            Device::Alternate(ref device) => device.adapter(),
        }
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    pub fn gl_api(&self) -> GLApi {
        match *self {
            Device::Default(ref device) => device.gl_api(),
            Device::Alternate(ref device) => device.gl_api(),
        }
    }
}

impl<Def, Alt> DeviceInterface for Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: ConnectionInterface<Device = Def>,
    Alt::Connection: ConnectionInterface<Device = Alt>,
{
    type Connection = Connection<Def, Alt>;

    // device.rs

    #[inline]
    fn connection(&self) -> Connection<Def, Alt> {
        Device::connection(self)
    }

    #[inline]
    fn adapter(&self) -> Adapter {
        Device::adapter(self)
    }

    #[inline]
    fn gl_api(&self) -> GLApi {
        Device::gl_api(self)
    }

    // context.rs

    #[inline]
    fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        Device::create_context_descriptor(self, attributes)
    }

    #[inline]
    fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        Device::create_context(self, descriptor, share_with)
    }

    #[inline]
    fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        Device::destroy_context(self, context)
    }

    #[inline]
    fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        Device::context_descriptor(self, context)
    }

    #[inline]
    fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        Device::make_context_current(self, context)
    }

    #[inline]
    fn make_no_context_current(&self) -> Result<(), Error> {
        Device::make_no_context_current(self)
    }

    #[inline]
    fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        Device::context_descriptor_attributes(self, context_descriptor)
    }

    #[inline]
    fn get_proc_address(&self, context: &Context, symbol_name: &str) -> *const c_void {
        Device::get_proc_address(self, context, symbol_name)
    }

    #[inline]
    fn bind_surface_to_context(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<(), (Error, Surface)> {
        Device::bind_surface_to_context(self, context, surface)
    }

    #[inline]
    fn unbind_surface_from_context(&self, context: &mut Context) -> Result<Option<Surface>, Error> {
        Device::unbind_surface_from_context(self, context)
    }

    #[inline]
    fn context_id(&self, context: &Context) -> ContextID {
        Device::context_id(self, context)
    }

    #[inline]
    fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        Device::context_surface_info(self, context)
    }

    // surface.rs

    #[inline]
    fn create_surface(
        &self,
        context: &Context,
        surface_access: SurfaceAccess,
        surface_type: SurfaceType<'_>,
    ) -> Result<Surface, Error> {
        Device::create_surface(self, context, surface_access, surface_type)
    }

    #[inline]
    fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        Device::create_surface_texture(self, context, surface)
    }

    #[inline]
    fn destroy_surface(&self, context: &mut Context, surface: &mut Surface) -> Result<(), Error> {
        Device::destroy_surface(self, context, surface)
    }

    #[inline]
    fn destroy_surface_texture(
        &self,
        context: &mut Context,
        surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        Device::destroy_surface_texture(self, context, surface_texture)
    }

    #[inline]
    fn surface_gl_texture_target(&self) -> u32 {
        Device::surface_gl_texture_target(self)
    }

    #[inline]
    fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        Device::present_bound_surface(self, context)
    }

    #[inline]
    fn resize_bound_surface(&self, context: &mut Context, size: Size2D<i32>) -> Result<(), Error> {
        Device::resize_bound_surface(self, context, size)
    }

    #[inline]
    fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        Device::present_surface(self, context, surface)
    }

    #[inline]
    fn resize_surface(
        &self,
        context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        Device::resize_surface(self, context, surface, size)
    }

    #[inline]
    fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        Device::surface_info(self, surface)
    }

    #[inline]
    fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> Option<Texture> {
        Device::surface_texture_object(self, surface_texture)
    }
}
