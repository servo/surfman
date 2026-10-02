//! A context abstraction that allows the choice of backends dynamically.

use euclid::default::Size2D;

use super::device::Device;
use crate::device::Device as DeviceInterface;
use crate::{
    Context, ContextAttributes, ContextDescriptor, ContextID, Error, Surface, SurfaceInfo,
};

use std::os::raw::c_void;

impl<Def, Alt> Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        match *self {
            Device::Default(ref device) => device.create_context_descriptor(attributes),
            Device::Alternate(ref device) => device.create_context_descriptor(attributes),
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        match self {
            Device::Default(device) => device.create_context(descriptor, share_with),
            Device::Alternate(device) => device.create_context(descriptor, share_with),
        }
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.destroy_context(context),
            Device::Alternate(device) => device.destroy_context(context),
        }
    }

    /// Returns the descriptor that this context was created with.
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        match self {
            Device::Default(device) => device.context_descriptor(context),
            Device::Alternate(device) => device.context_descriptor(context),
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.make_context_current(context),
            Device::Alternate(device) => device.make_context_current(context),
        }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.make_no_context_current(),
            Device::Alternate(device) => device.make_no_context_current(),
        }
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
        match self {
            Device::Default(device) => device.bind_surface_to_context(context, surface),
            Device::Alternate(device) => device.bind_surface_to_context(context, surface),
        }
    }

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context,
    ) -> Result<Option<Surface>, Error> {
        match self {
            Device::Default(device) => device.unbind_surface_from_context(context),
            Device::Alternate(device) => device.unbind_surface_from_context(context),
        }
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.present_bound_surface(context),
            Device::Alternate(device) => device.present_bound_surface(context),
        }
    }

    /// Resizes the currently bound surface.
    pub fn resize_bound_surface(
        &self,
        context: &mut Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        match self {
            Device::Default(device) => device.resize_bound_surface(context, size),
            Device::Alternate(device) => device.resize_bound_surface(context, size),
        }
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        match self {
            Device::Default(device) => device.context_descriptor_attributes(context_descriptor),
            Device::Alternate(device) => device.context_descriptor_attributes(context_descriptor),
        }
    }

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    pub fn get_proc_address(&self, context: &Context, symbol_name: &str) -> *const c_void {
        match self {
            Device::Default(device) => device.get_proc_address(context, symbol_name),
            Device::Alternate(device) => device.get_proc_address(context, symbol_name),
        }
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    pub fn context_id(&self, context: &Context) -> ContextID {
        match self {
            Device::Default(device) => device.context_id(context),
            Device::Alternate(device) => device.context_id(context),
        }
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match self {
            Device::Default(device) => device.context_surface_info(context),
            Device::Alternate(device) => device.context_surface_info(context),
        }
    }
}
