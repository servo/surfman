// surfman/surfman/src/implementation/device.rs
//
//! This is an included private module that automatically produces the implementation of the
//! `Device` trait for a backend.

use crate::device::Device as DeviceInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::{ContextAttributes, ContextID, Error, GLApi, SurfaceAccess, SurfaceInfo, SurfaceType};
use super::super::adapter::Adapter;
use super::super::connection::Connection;
use super::super::context::{Context, ContextDescriptor};
use super::super::device::Device;
use super::super::surface::{NativeWidget, Surface, SurfaceTexture};

use std::os::raw::c_void;

impl DeviceInterface for Device {
    type Connection = Connection;
    type Context = Context;
    type ContextDescriptor = ContextDescriptor;
    type Surface = Surface;
    type SurfaceTexture = SurfaceTexture;

    // device.rs

    #[inline]
    fn new(connection: &Connection, adapter: &Adapter) -> Result<Self, Error> {
        Device::new(connection, adapter)
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
    fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                 -> Result<Self::ContextDescriptor, Error> {
        Device::create_context_descriptor(self, attributes)
    }

    #[inline]
    unsafe fn from_current_context() -> Result<(Self, Self::Context), Error> {
        Device::from_current_context()
    }

    #[inline]
    fn create_context(&mut self, descriptor: &Self::ContextDescriptor)
                      -> Result<Self::Context, Error> {
        Device::create_context(self, descriptor)
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
    fn context_descriptor_attributes(&self, context_descriptor: &Self::ContextDescriptor)
                                     -> ContextAttributes {
        Device::context_descriptor_attributes(self, context_descriptor)
    }

    #[inline]
    fn get_proc_address(&self, context: &Self::Context, symbol_name: &str) -> *const c_void {
        Device::get_proc_address(self, context, symbol_name)
    }

    #[inline]
    fn bind_surface_to_context(&self, context: &mut Self::Context, surface: Self::Surface)
                               -> Result<(), Error> {
        Device::bind_surface_to_context(self, context, surface)
    }

    #[inline]
    fn unbind_surface_from_context(&self, context: &mut Self::Context)
                                   -> Result<Option<Self::Surface>, Error> {
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

    // surface.rs

    #[inline]
    fn create_surface(&mut self,
                      context: &Self::Context,
                      surface_access: SurfaceAccess,
                      surface_type: &SurfaceType<NativeWidget>)
                      -> Result<Self::Surface, Error> {
        Device::create_surface(self, context, surface_access, surface_type)
    }

    #[inline]
    fn create_surface_texture(&self, context: &mut Self::Context, surface: Self::Surface)
                              -> Result<Self::SurfaceTexture, Error> {
        Device::create_surface_texture(self, context, surface)
    }

    #[inline]
    fn destroy_surface(&self, context: &mut Self::Context, surface: Self::Surface)
                       -> Result<(), Error> {
        Device::destroy_surface(self, context, surface)
    }

    #[inline]
    fn destroy_surface_texture(&self,
                               context: &mut Self::Context,
                               surface_texture: Self::SurfaceTexture)
                               -> Result<Self::Surface, Error> {
        Device::destroy_surface_texture(self, context, surface_texture)
    }

    #[inline]
    fn surface_gl_texture_target(&self) -> GLenum {
        Device::surface_gl_texture_target(self)
    }

    #[inline]
    fn present_surface(&self, context: &Self::Context, surface: &mut Self::Surface)
                       -> Result<(), Error> {
        Device::present_surface(self, context, surface)
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
