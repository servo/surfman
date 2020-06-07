// surfman/surfman/src/platform/generic/multi/context.rs
//
//! A context abstraction that allows the choice of backends dynamically.

use super::device::Device;
use super::surface::Surface;
use crate::device::Device as DeviceInterface;
use crate::{ContextAttributes, ContextID, Error, SurfaceInfo};

use std::os::raw::c_void;

/// Represents an OpenGL rendering context.
///
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
///
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
///
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
///
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub enum Context<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default rendering context type.
    Default(Def::Context),
    /// The alternate rendering context type.
    Alternate(Alt::Context),
}

/// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
///
/// These are local to a device.
#[derive(Clone)]
pub enum ContextDescriptor<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default context descriptor type.
    Default(Def::ContextDescriptor),
    /// The alternate context descriptor type.
    Alternate(Alt::ContextDescriptor),
}

/// Wraps a platform-specific native context.
pub enum NativeContext<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default context type.
    Default(Def::NativeContext),
    /// The alternate context type.
    Alternate(Alt::NativeContext),
}

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
    ) -> Result<ContextDescriptor<Def, Alt>, Error> {
        match *self {
            Device::Default(ref device) => device
                .create_context_descriptor(attributes)
                .map(ContextDescriptor::Default),
            Device::Alternate(ref device) => device
                .create_context_descriptor(attributes)
                .map(ContextDescriptor::Alternate),
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(
        &mut self,
        descriptor: &ContextDescriptor<Def, Alt>,
        share_with: Option<&Context<Def, Alt>>,
    ) -> Result<Context<Def, Alt>, Error> {
        match (&mut *self, descriptor) {
            (&mut Device::Default(ref mut device), &ContextDescriptor::Default(ref descriptor)) => {
                let shared = match share_with {
                    Some(&Context::Default(ref other)) => Some(other),
                    Some(_) => {
                        return Err(Error::IncompatibleSharedContext);
                    }
                    None => None,
                };
                device
                    .create_context(descriptor, shared)
                    .map(Context::Default)
            }
            (
                &mut Device::Alternate(ref mut device),
                &ContextDescriptor::Alternate(ref descriptor),
            ) => {
                let shared = match share_with {
                    Some(&Context::Alternate(ref other)) => Some(other),
                    Some(_) => {
                        return Err(Error::IncompatibleSharedContext);
                    }
                    None => None,
                };
                device
                    .create_context(descriptor, shared)
                    .map(Context::Alternate)
            }
            _ => Err(Error::IncompatibleContextDescriptor),
        }
    }

    /// Wraps an existing native context in a `Context` object.
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext<Def, Alt>,
    ) -> Result<Context<Def, Alt>, Error> {
        match self {
            &Device::Default(ref device) => match native_context {
                NativeContext::Default(native_context) => device
                    .create_context_from_native_context(native_context)
                    .map(Context::Default),
                _ => Err(Error::IncompatibleNativeContext),
            },
            &Device::Alternate(ref device) => match native_context {
                NativeContext::Alternate(native_context) => device
                    .create_context_from_native_context(native_context)
                    .map(Context::Alternate),
                _ => Err(Error::IncompatibleNativeContext),
            },
        }
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context<Def, Alt>) -> Result<(), Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                device.destroy_context(context)
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                device.destroy_context(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    /// Returns the native context underlying this context.
    pub fn native_context(&self, context: &Context<Def, Alt>) -> NativeContext<Def, Alt> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                NativeContext::Default(device.native_context(context))
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                NativeContext::Alternate(device.native_context(context))
            }
            _ => panic!("Incompatible context!"),
        }
    }

    /// Returns the descriptor that this context was created with.
    pub fn context_descriptor(&self, context: &Context<Def, Alt>) -> ContextDescriptor<Def, Alt> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                ContextDescriptor::Default(device.context_descriptor(context))
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                ContextDescriptor::Alternate(device.context_descriptor(context))
            }
            _ => panic!("Incompatible context!"),
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context<Def, Alt>) -> Result<(), Error> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                device.make_context_current(context)
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                device.make_context_current(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        match self {
            &Device::Default(ref device) => device.make_no_context_current(),
            &Device::Alternate(ref device) => device.make_no_context_current(),
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
        context: &mut Context<Def, Alt>,
        surface: Surface<Def, Alt>,
    ) -> Result<(), (Error, Surface<Def, Alt>)> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => match surface
            {
                Surface::Default(surface) => device
                    .bind_surface_to_context(context, surface)
                    .map_err(|(err, surface)| (err, Surface::Default(surface))),
                _ => Err((Error::IncompatibleSurface, surface)),
            },
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface {
                    Surface::Alternate(surface) => device
                        .bind_surface_to_context(context, surface)
                        .map_err(|(err, surface)| (err, Surface::Alternate(surface))),
                    _ => Err((Error::IncompatibleSurface, surface)),
                }
            }
            _ => Err((Error::IncompatibleContext, surface)),
        }
    }

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context<Def, Alt>,
    ) -> Result<Option<Surface<Def, Alt>>, Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => device
                .unbind_surface_from_context(context)
                .map(|surface| surface.map(Surface::Default)),
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => device
                .unbind_surface_from_context(context)
                .map(|surface| surface.map(Surface::Alternate)),
            _ => Err(Error::IncompatibleContext),
        }
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor<Def, Alt>,
    ) -> ContextAttributes {
        match (self, context_descriptor) {
            (&Device::Default(ref device), &ContextDescriptor::Default(ref context_descriptor)) => {
                device.context_descriptor_attributes(context_descriptor)
            }
            (
                &Device::Alternate(ref device),
                &ContextDescriptor::Alternate(ref context_descriptor),
            ) => device.context_descriptor_attributes(context_descriptor),
            _ => panic!("Incompatible context!"),
        }
    }

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    pub fn get_proc_address(
        &self,
        context: &Context<Def, Alt>,
        symbol_name: &str,
    ) -> *const c_void {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                device.get_proc_address(context, symbol_name)
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                device.get_proc_address(context, symbol_name)
            }
            _ => panic!("Incompatible context!"),
        }
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    pub fn context_id(&self, context: &Context<Def, Alt>) -> ContextID {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                device.context_id(context)
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                device.context_id(context)
            }
            _ => panic!("Incompatible context!"),
        }
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(
        &self,
        context: &Context<Def, Alt>,
    ) -> Result<Option<SurfaceInfo>, Error> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                device.context_surface_info(context)
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                device.context_surface_info(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }
}
