// surfman/surfman/src/implementation/device.rs
//
//! This is an included private module that automatically produces the implementation of the
//! `Device` trait for a backend.

use super::super::connection::Connection;
use super::super::context::{Context, ContextDescriptor, NativeContext};
use super::super::device::{Adapter, Device};
use super::super::surface::{NativeWidget, Surface, SurfaceTexture};
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::{ContextAttributes, ContextID, Error, GLApi, SurfaceAccess, SurfaceInfo, SurfaceType};
use euclid::default::Size2D;

use std::os::raw::c_void;

#[deny(unconditional_recursion)]
impl DeviceInterface for Device {
    type Connection = Connection;
    type Context = Context;
    type ContextDescriptor = ContextDescriptor;
    type NativeContext = NativeContext;
    type Surface = Surface;
    type SurfaceTexture = SurfaceTexture;

    // device.rs

    /// Returns the native device associated with this device.
    #[inline]
    fn native_device(&self) -> <Self::Connection as ConnectionInterface>::NativeDevice {
        Device::native_device(self)
    }

    #[inline]
    fn connection(&self) -> Connection {
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
    ) -> Result<Self::ContextDescriptor, Error> {
        Device::create_context_descriptor(self, attributes)
    }

    #[inline]
    fn create_context(
        &mut self,
        descriptor: &Self::ContextDescriptor,
        share_with: Option<&Self::Context>,
    ) -> Result<Self::Context, Error> {
        Device::create_context(self, descriptor, share_with)
    }

    #[inline]
    unsafe fn create_context_from_native_context(
        &self,
        native_context: Self::NativeContext,
    ) -> Result<Self::Context, Error> {
        Device::create_context_from_native_context(self, native_context)
    }

    #[inline]
    fn destroy_context(&self, context: &mut Self::Context) -> Result<(), Error> {
        Device::destroy_context(self, context)
    }

    #[inline]
    fn context_descriptor(&self, context: &Self::Context) -> Self::ContextDescriptor {
        Device::context_descriptor(self, context)
    }

    #[inline]
    fn make_context_current(&self, context: &Self::Context) -> Result<(), Error> {
        Device::make_context_current(self, context)
    }

    #[inline]
    fn make_no_context_current(&self) -> Result<(), Error> {
        Device::make_no_context_current(self)
    }

    #[inline]
    fn context_descriptor_attributes(
        &self,
        context_descriptor: &Self::ContextDescriptor,
    ) -> ContextAttributes {
        Device::context_descriptor_attributes(self, context_descriptor)
    }

    #[inline]
    fn get_proc_address(&self, context: &Self::Context, symbol_name: &str) -> *const c_void {
        Device::get_proc_address(self, context, symbol_name)
    }

    #[inline]
    fn bind_surface_to_context(
        &self,
        context: &mut Self::Context,
        surface: Self::Surface,
    ) -> Result<(), (Error, Self::Surface)> {
        Device::bind_surface_to_context(self, context, surface)
    }

    #[inline]
    fn unbind_surface_from_context(
        &self,
        context: &mut Self::Context,
    ) -> Result<Option<Self::Surface>, Error> {
        Device::unbind_surface_from_context(self, context)
    }

    #[inline]
    fn context_id(&self, context: &Self::Context) -> ContextID {
        Device::context_id(self, context)
    }

    #[inline]
    fn context_surface_info(&self, context: &Self::Context) -> Result<Option<SurfaceInfo>, Error> {
        Device::context_surface_info(self, context)
    }

    #[inline]
    fn native_context(&self, context: &Self::Context) -> Self::NativeContext {
        Device::native_context(self, context)
    }

    // surface.rs

    #[inline]
    fn create_surface(
        &mut self,
        context: &Self::Context,
        surface_access: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Self::Surface, Error> {
        Device::create_surface(self, context, surface_access, surface_type)
    }

    #[inline]
    fn create_surface_texture(
        &self,
        context: &mut Self::Context,
        surface: Self::Surface,
    ) -> Result<Self::SurfaceTexture, (Error, Self::Surface)> {
        Device::create_surface_texture(self, context, surface)
    }

    #[inline]
    fn destroy_surface(
        &self,
        context: &mut Self::Context,
        surface: &mut Self::Surface,
    ) -> Result<(), Error> {
        Device::destroy_surface(self, context, surface)
    }

    #[inline]
    fn destroy_surface_texture(
        &self,
        context: &mut Self::Context,
        surface_texture: Self::SurfaceTexture,
    ) -> Result<Self::Surface, (Error, Self::SurfaceTexture)> {
        Device::destroy_surface_texture(self, context, surface_texture)
    }

    #[inline]
    fn surface_gl_texture_target(&self) -> GLenum {
        Device::surface_gl_texture_target(self)
    }

    #[inline]
    fn present_surface(
        &self,
        context: &Self::Context,
        surface: &mut Self::Surface,
    ) -> Result<(), Error> {
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
    fn surface_info(&self, surface: &Self::Surface) -> SurfaceInfo {
        Device::surface_info(self, surface)
    }

    #[inline]
    fn surface_texture_object(&self, surface_texture: &Self::SurfaceTexture) -> GLuint {
        Device::surface_texture_object(self, surface_texture)
    }
}
