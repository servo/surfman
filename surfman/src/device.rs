// surfman/surfman/src/device.rs
//
//! The abstract interface that all devices conform to.

use crate::{ContextAttributes, ContextID, Error, GLApi, SurfaceAccess, SurfaceInfo, SurfaceType};
use crate::gl::types::{GLenum, GLuint};
use super::connection::Connection as ConnectionInterface;

use std::os::raw::c_void;

pub trait Device: Sized where Self::Connection: ConnectionInterface {
    type Connection;
    type Context;
    type ContextDescriptor;
    type NativeWidget;
    type Surface;
    type SurfaceTexture;

    // device.rs
    fn new(connection: &Self::Connection,
           adapter: &<Self::Connection as ConnectionInterface>::Adapter)
           -> Result<Self, Error>;
    fn connection(&self) -> Self::Connection;
    fn adapter(&self) -> <Self::Connection as ConnectionInterface>::Adapter;
    fn gl_api() -> GLApi;

    // context.rs
    fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                 -> Result<Self::ContextDescriptor, Error>;
    unsafe fn from_current_context() -> Result<(Self, Self::Context), Error>;
    fn create_context(&mut self, descriptor: &Self::ContextDescriptor)
                      -> Result<Self::Context, Error>;
    fn destroy_context(&self, context: &mut Self::Context) -> Result<(), Error>;
    fn context_descriptor(&self, context: &Self::Context) -> Self::ContextDescriptor;
    fn make_context_current(&self, context: &Self::Context) -> Result<(), Error>;
    fn make_no_context_current(&self) -> Result<(), Error>;
    fn context_descriptor_attributes(&self, context_descriptor: &Self::ContextDescriptor)
                                     -> ContextAttributes;
    fn get_proc_address(&self, context: &Self::Context, symbol_name: &str) -> *const c_void;
    fn bind_surface_to_context(&self, context: &mut Self::Context, surface: Self::Surface)
                               -> Result<(), Error>;
    fn unbind_surface_from_context(&self, context: &mut Self::Context)
                                   -> Result<Option<Self::Surface>, Error>;
    fn context_id(&self, context: &Self::Context) -> ContextID;
    fn context_surface_info(&self, context: &Self::Context) -> Result<Option<SurfaceInfo>, Error>;

    // surface.rs
    fn create_surface(&mut self,
                      context: &Self::Context,
                      surface_access: SurfaceAccess,
                      surface_type: &SurfaceType<Self::NativeWidget>)
                      -> Result<Self::Surface, Error>;
    fn create_surface_texture(&self, context: &mut Self::Context, surface: Self::Surface)
                              -> Result<Self::SurfaceTexture, Error>;
    fn destroy_surface(&self, context: &mut Self::Context, surface: Self::Surface)
                       -> Result<(), Error>;
    fn destroy_surface_texture(&self,
                               context: &mut Self::Context,
                               surface_texture: Self::SurfaceTexture)
                               -> Result<Self::Surface, Error>;
    fn surface_gl_texture_target(&self) -> GLenum;
    fn present_surface(&self, context: &Self::Context, surface: &mut Self::Surface)
                       -> Result<(), Error>;
    fn surface_info(&self, surface: &Self::Surface) -> SurfaceInfo;
    fn surface_texture_object(&self, surface_texture: &Self::SurfaceTexture) -> GLuint;
}
