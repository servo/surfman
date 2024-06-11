// surfman/surfman/src/platform/generic/multi/device.rs
//
//! A device abstraction that allows the choice of backends dynamically.

use super::connection::Connection;
use super::context::{Context, ContextDescriptor, NativeContext};
use super::surface::{NativeWidget, Surface, SurfaceTexture};
use crate::connection::Connection as ConnectionInterface;
use crate::context::ContextAttributes;
use crate::device::Device as DeviceInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::{ContextID, Error, GLApi, SurfaceAccess, SurfaceInfo, SurfaceType};
use euclid::default::Size2D;

use std::os::raw::c_void;

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
pub enum Adapter<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default adapter type.
    Default(<Def::Connection as ConnectionInterface>::Adapter),
    /// The alternate adapter type.
    Alternate(<Alt::Connection as ConnectionInterface>::Adapter),
}

impl<Def, Alt> Clone for Adapter<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    <Def::Connection as ConnectionInterface>::Adapter: Clone,
    <Alt::Connection as ConnectionInterface>::Adapter: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Adapter::Default(ref adapter) => Adapter::Default(adapter.clone()),
            Adapter::Alternate(ref adapter) => Adapter::Alternate(adapter.clone()),
        }
    }
}

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

/// Represents a native platform-specific device.
pub enum NativeDevice<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default native device type.
    Default(<Def::Connection as ConnectionInterface>::NativeDevice),
    /// The alternate native device type.
    Alternate(<Alt::Connection as ConnectionInterface>::NativeDevice),
}

impl<Def, Alt> Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
    Def::Connection: ConnectionInterface,
    Alt::Connection: ConnectionInterface,
{
    /// Returns the native device underlying this device.
    pub fn native_device(&self) -> NativeDevice<Def, Alt> {
        match *self {
            Device::Default(ref device) => NativeDevice::Default(device.native_device()),
            Device::Alternate(ref device) => NativeDevice::Alternate(device.native_device()),
        }
    }

    /// Returns the display server connection that this device was created with.
    pub fn connection(&self) -> Connection<Def, Alt> {
        match *self {
            Device::Default(ref device) => Connection::Default(device.connection()),
            Device::Alternate(ref device) => Connection::Alternate(device.connection()),
        }
    }

    /// Returns the adapter that this device was created with.
    pub fn adapter(&self) -> Adapter<Def, Alt> {
        match *self {
            Device::Default(ref device) => Adapter::Default(device.adapter()),
            Device::Alternate(ref device) => Adapter::Alternate(device.adapter()),
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
    type Context = Context<Def, Alt>;
    type ContextDescriptor = ContextDescriptor<Def, Alt>;
    type NativeContext = NativeContext<Def, Alt>;
    type Surface = Surface<Def, Alt>;
    type SurfaceTexture = SurfaceTexture<Def, Alt>;

    // device.rs

    #[inline]
    fn native_device(&self) -> NativeDevice<Def, Alt> {
        Device::native_device(self)
    }

    #[inline]
    fn connection(&self) -> Connection<Def, Alt> {
        Device::connection(self)
    }

    #[inline]
    fn adapter(&self) -> Adapter<Def, Alt> {
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
    ) -> Result<Self::ContextDescriptor, Error> {
        Device::create_context_descriptor(self, attributes)
    }

    #[inline]
    fn create_context(
        &mut self,
        descriptor: &ContextDescriptor<Def, Alt>,
        share_with: Option<&Context<Def, Alt>>,
    ) -> Result<Context<Def, Alt>, Error> {
        Device::create_context(self, descriptor, share_with)
    }

    #[inline]
    unsafe fn create_context_from_native_context(
        &self,
        native_context: Self::NativeContext,
    ) -> Result<Context<Def, Alt>, Error> {
        Device::create_context_from_native_context(self, native_context)
    }

    #[inline]
    fn destroy_context(&self, context: &mut Context<Def, Alt>) -> Result<(), Error> {
        Device::destroy_context(self, context)
    }

    #[inline]
    fn native_context(&self, context: &Context<Def, Alt>) -> Self::NativeContext {
        Device::native_context(self, context)
    }

    #[inline]
    fn context_descriptor(&self, context: &Context<Def, Alt>) -> Self::ContextDescriptor {
        Device::context_descriptor(self, context)
    }

    #[inline]
    fn make_context_current(&self, context: &Context<Def, Alt>) -> Result<(), Error> {
        Device::make_context_current(self, context)
    }

    #[inline]
    fn make_no_context_current(&self) -> Result<(), Error> {
        Device::make_no_context_current(self)
    }

    #[inline]
    fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor<Def, Alt>,
    ) -> ContextAttributes {
        Device::context_descriptor_attributes(self, context_descriptor)
    }

    #[inline]
    fn get_proc_address(&self, context: &Context<Def, Alt>, symbol_name: &str) -> *const c_void {
        Device::get_proc_address(self, context, symbol_name)
    }

    #[inline]
    fn bind_surface_to_context(
        &self,
        context: &mut Context<Def, Alt>,
        surface: Surface<Def, Alt>,
    ) -> Result<(), (Error, Surface<Def, Alt>)> {
        Device::bind_surface_to_context(self, context, surface)
    }

    #[inline]
    fn unbind_surface_from_context(
        &self,
        context: &mut Context<Def, Alt>,
    ) -> Result<Option<Surface<Def, Alt>>, Error> {
        Device::unbind_surface_from_context(self, context)
    }

    #[inline]
    fn context_id(&self, context: &Context<Def, Alt>) -> ContextID {
        Device::context_id(self, context)
    }

    #[inline]
    fn context_surface_info(
        &self,
        context: &Context<Def, Alt>,
    ) -> Result<Option<SurfaceInfo>, Error> {
        Device::context_surface_info(self, context)
    }

    // surface.rs

    #[inline]
    fn create_surface(
        &mut self,
        context: &Context<Def, Alt>,
        surface_access: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget<Def, Alt>>,
    ) -> Result<Surface<Def, Alt>, Error> {
        Device::create_surface(self, context, surface_access, surface_type)
    }

    #[inline]
    fn create_surface_texture(
        &self,
        context: &mut Context<Def, Alt>,
        surface: Surface<Def, Alt>,
    ) -> Result<SurfaceTexture<Def, Alt>, (Error, Surface<Def, Alt>)> {
        Device::create_surface_texture(self, context, surface)
    }

    #[inline]
    fn destroy_surface(
        &self,
        context: &mut Context<Def, Alt>,
        surface: &mut Surface<Def, Alt>,
    ) -> Result<(), Error> {
        Device::destroy_surface(self, context, surface)
    }

    #[inline]
    fn destroy_surface_texture(
        &self,
        context: &mut Context<Def, Alt>,
        surface_texture: SurfaceTexture<Def, Alt>,
    ) -> Result<Surface<Def, Alt>, (Error, SurfaceTexture<Def, Alt>)> {
        Device::destroy_surface_texture(self, context, surface_texture)
    }

    #[inline]
    fn surface_gl_texture_target(&self) -> GLenum {
        Device::surface_gl_texture_target(self)
    }

    #[inline]
    fn present_surface(
        &self,
        context: &Context<Def, Alt>,
        surface: &mut Surface<Def, Alt>,
    ) -> Result<(), Error> {
        Device::present_surface(self, context, surface)
    }

    #[inline]
    fn resize_surface(
        &self,
        context: &Context<Def, Alt>,
        surface: &mut Surface<Def, Alt>,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        Device::resize_surface(self, context, surface, size)
    }

    #[inline]
    fn surface_info(&self, surface: &Surface<Def, Alt>) -> SurfaceInfo {
        Device::surface_info(self, surface)
    }

    #[inline]
    fn surface_texture_object(&self, surface_texture: &SurfaceTexture<Def, Alt>) -> GLuint {
        Device::surface_texture_object(self, surface_texture)
    }
}
