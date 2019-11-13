// surfman/surfman/src/platform/generic/multi/context.rs
//
//! A context abstraction that allows the choice of backends dynamically.

use crate::{ContextAttributes, ContextID, Error, SurfaceInfo};
use crate::device::Device as DeviceInterface;
use super::device::Device;
use super::surface::Surface;

use std::os::raw::c_void;

pub enum Context<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(Def::Context),
    Alternate(Alt::Context),
}

#[derive(Clone)]
pub enum ContextDescriptor<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(Def::ContextDescriptor),
    Alternate(Alt::ContextDescriptor),
}

impl<Def, Alt> Device<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor<Def, Alt>, Error> {
        match *self {
            Device::Default(ref device) => {
                device.create_context_descriptor(attributes).map(ContextDescriptor::Default)
            }
            Device::Alternate(ref device) => {
                device.create_context_descriptor(attributes).map(ContextDescriptor::Alternate)
            }
        }
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor<Def, Alt>)
                          -> Result<Context<Def, Alt>, Error> {
        match (&mut *self, descriptor) {
            (&mut Device::Default(ref mut device),
             &ContextDescriptor::Default(ref descriptor)) => {
                 device.create_context(descriptor).map(Context::Default)
            }
            (&mut Device::Alternate(ref mut device),
             &ContextDescriptor::Alternate(ref descriptor)) => {
                device.create_context(descriptor).map(Context::Alternate)
            }
            _ => Err(Error::IncompatibleContextDescriptor),
        }
    }

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

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        match self {
            &Device::Default(ref device) => {
                device.make_no_context_current()
            }
            &Device::Alternate(ref device) => {
                device.make_no_context_current()
            }
        }
    }

    pub fn bind_surface_to_context(&self,
                                   context: &mut Context<Def, Alt>,
                                   surface: Surface<Def, Alt>)
                                   -> Result<(), (Error, Surface<Def, Alt>)> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match surface {
                    Surface::Default(surface) => {
                        device.bind_surface_to_context(context, surface).map_err(|(err, surface)| {
                            (err, Surface::Default(surface))
                        })
                    }
                    _ => Err((Error::IncompatibleSurface, surface)),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface {
                    Surface::Alternate(surface) => {
                        device.bind_surface_to_context(context, surface).map_err(|(err, surface)| {
                            (err, Surface::Alternate(surface))
                        })
                    }
                    _ => Err((Error::IncompatibleSurface, surface)),
                }
            }
            _ => Err((Error::IncompatibleContext, surface)),
        }
    }

    pub fn unbind_surface_from_context(&self, context: &mut Context<Def, Alt>)
                                       -> Result<Option<Surface<Def, Alt>>, Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                device.unbind_surface_from_context(context).map(|surface| {
                    surface.map(Surface::Default)
                })
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                device.unbind_surface_from_context(context).map(|surface| {
                    surface.map(Surface::Alternate)
                })
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor<Def, Alt>)
                                         -> ContextAttributes {
        match (self, context_descriptor) {
            (&Device::Default(ref device),
             &ContextDescriptor::Default(ref context_descriptor)) => {
                device.context_descriptor_attributes(context_descriptor)
            }
            (&Device::Alternate(ref device),
             &ContextDescriptor::Alternate(ref context_descriptor)) => {
                device.context_descriptor_attributes(context_descriptor)
            }
            _ => panic!("Incompatible context!")
        }
    }

    pub fn get_proc_address(&self, context: &Context<Def, Alt>, symbol_name: &str)
                            -> *const c_void {
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

    pub fn context_surface_info(&self, context: &Context<Def, Alt>)
                                -> Result<Option<SurfaceInfo>, Error> {
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
