// surfman/surfman/src/device.rs
//
//! The abstract interface that all devices conform to.

use super::connection::Connection as ConnectionInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::{ContextAttributes, ContextID, Error, GLApi, SurfaceAccess, SurfaceInfo, SurfaceType};
use euclid::default::Size2D;

use std::os::raw::c_void;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub trait Device: Sized
where
    Self::Connection: ConnectionInterface,
{
    /// The connection type associated with this device.
    type Connection;
    /// The context type associated with this device.
    type Context;
    /// The context descriptor type associated with this device.
    type ContextDescriptor;
    /// The native context type associated with this device.
    type NativeContext;
    /// The surface type associated with this device.
    type Surface;
    /// The surface texture type associated with this device.
    type SurfaceTexture;

    // device.rs

    /// Returns the native device associated with this device.
    fn native_device(&self) -> <Self::Connection as ConnectionInterface>::NativeDevice;

    /// Returns the display server connection that this device was created with.
    fn connection(&self) -> Self::Connection;

    /// Returns the adapter that this device was created with.
    fn adapter(&self) -> <Self::Connection as ConnectionInterface>::Adapter;

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    fn gl_api(&self) -> GLApi;

    // context.rs

    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<Self::ContextDescriptor, Error>;

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    fn create_context(
        &mut self,
        descriptor: &Self::ContextDescriptor,
        share_with: Option<&Self::Context>,
    ) -> Result<Self::Context, Error>;

    /// Wraps a native context object in an OpenGL context.
    unsafe fn create_context_from_native_context(
        &self,
        native_context: Self::NativeContext,
    ) -> Result<Self::Context, Error>;

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    fn destroy_context(&self, context: &mut Self::Context) -> Result<(), Error>;

    /// Returns the descriptor that this context was created with.
    fn context_descriptor(&self, context: &Self::Context) -> Self::ContextDescriptor;

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    fn make_context_current(&self, context: &Self::Context) -> Result<(), Error>;

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    fn make_no_context_current(&self) -> Result<(), Error>;

    /// Returns the attributes that the context descriptor was created with.
    fn context_descriptor_attributes(
        &self,
        context_descriptor: &Self::ContextDescriptor,
    ) -> ContextAttributes;

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    fn get_proc_address(&self, context: &Self::Context, symbol_name: &str) -> *const c_void;

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
    fn bind_surface_to_context(
        &self,
        context: &mut Self::Context,
        surface: Self::Surface,
    ) -> Result<(), (Error, Self::Surface)>;

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    fn unbind_surface_from_context(
        &self,
        context: &mut Self::Context,
    ) -> Result<Option<Self::Surface>, Error>;

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    fn context_id(&self, context: &Self::Context) -> ContextID;

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    fn context_surface_info(&self, context: &Self::Context) -> Result<Option<SurfaceInfo>, Error>;

    /// Returns the native context associated with the given context.
    fn native_context(&self, context: &Self::Context) -> Self::NativeContext;

    // surface.rs

    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    fn create_surface(
        &mut self,
        context: &Self::Context,
        surface_access: SurfaceAccess,
        surface_type: SurfaceType<<Self::Connection as ConnectionInterface>::NativeWidget>,
    ) -> Result<Self::Surface, Error>;

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
    fn create_surface_texture(
        &self,
        context: &mut Self::Context,
        surface: Self::Surface,
    ) -> Result<Self::SurfaceTexture, (Error, Self::Surface)>;

    /// Destroys a surface.
    ///
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    fn destroy_surface(
        &self,
        context: &mut Self::Context,
        surface: &mut Self::Surface,
    ) -> Result<(), Error>;

    /// Destroys a surface texture and returns the underlying surface.
    ///
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    ///
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    fn destroy_surface_texture(
        &self,
        context: &mut Self::Context,
        surface_texture: Self::SurfaceTexture,
    ) -> Result<Self::Surface, (Error, Self::SurfaceTexture)>;

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    fn surface_gl_texture_target(&self) -> GLenum;

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    fn present_surface(
        &self,
        context: &Self::Context,
        surface: &mut Self::Surface,
    ) -> Result<(), Error>;

    /// Resizes a widget surface.
    fn resize_surface(
        &self,
        context: &Self::Context,
        surface: &mut Self::Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error>;

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    fn surface_info(&self, surface: &Self::Surface) -> SurfaceInfo;

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    fn surface_texture_object(&self, surface_texture: &Self::SurfaceTexture) -> GLuint;
}
