//! A context abstraction that can switch between hardware and software rendering.

use crate::gl::types::GLuint;
use crate::platform::default::context::Context as HWContext;
use crate::platform::default::context::ContextDescriptor as HWContextDescriptor;
use crate::platform::default::surface::SurfaceType as HWSurfaceType;
use crate::platform::default::device::Device as HWDevice;
use crate::platform::generic::osmesa::context::Context as OSMesaContext;
use crate::platform::generic::osmesa::context::ContextDescriptor as OSMesaContextDescriptor;
use crate::platform::generic::osmesa::device::Device as OSMesaDevice;
use crate::{ContextAttributes, Error, SurfaceID};
use super::device::Device;
use super::surface::Surface;
use super::surface::SurfaceType;

use euclid::default::Size2D;
use std::os::raw::c_void;

pub enum Context {
    Hardware(HWContext),
    Software(OSMesaContext),
}

#[derive(Clone)]
pub enum ContextDescriptor {
    Hardware(HWContextDescriptor),
    Software(OSMesaContextDescriptor),
}

impl Device {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        match *self {
            Device::Hardware(ref device) => {
                device.create_context_descriptor(attributes).map(ContextDescriptor::Hardware)
            }
            Device::Software(ref device) => {
                device.create_context_descriptor(attributes).map(ContextDescriptor::Software)
            }
        }
    }

    /// Opens the device and context corresponding to the current hardware context.
    pub unsafe fn from_current_hardware_context() -> Result<(Device, Context), Error> {
        HWDevice::from_current_context().map(|(device, context)| {
            (Device::Hardware(device), Context::Hardware(context))
        })
    }

    /// Opens the device and context corresponding to the current software context.
    pub unsafe fn from_current_software_context() -> Result<(Device, Context), Error> {
        OSMesaDevice::from_current_context().map(|(device, context)| {
            (Device::Software(device), Context::Software(context))
        })
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor, surface_type: &SurfaceType)
                          -> Result<Context, Error> {
        match (&mut *self, descriptor) {
            (&mut Device::Hardware(ref mut device),
             &ContextDescriptor::Hardware(ref descriptor)) => {
                 let ref surface_type = HWSurfaceType::from(*surface_type);
                 device.create_context(descriptor, surface_type).map(Context::Hardware)
            }
            (&mut Device::Software(ref mut device),
             &ContextDescriptor::Software(ref descriptor)) => {
                 device.create_context(descriptor, surface_type).map(Context::Software)
            }
            _ => Err(Error::IncompatibleContextDescriptor),
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        match (self, &mut *context) {
            (&Device::Hardware(ref device), &mut Context::Hardware(ref mut context)) => {
                device.destroy_context(context)
            }
            (&Device::Software(ref device), &mut Context::Software(ref mut context)) => {
                device.destroy_context(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                ContextDescriptor::Hardware(device.context_descriptor(context))
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                ContextDescriptor::Software(device.context_descriptor(context))
            }
            _ => panic!("Incompatible context!"),
        }
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                device.make_context_current(context)
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                device.make_context_current(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                device.make_no_context_current()
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                device.make_no_context_current()
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        match (self, &mut *context) {
            (&Device::Hardware(ref device), &mut Context::Hardware(ref mut context)) => {
                match new_surface {
                    Surface::Hardware(new_surface) => {
                        device.replace_context_surface(context, new_surface).map(Surface::Hardware)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Software(ref device), &mut Context::Software(ref mut context)) => {
                match new_surface {
                    Surface::Software(new_surface) => {
                        device.replace_context_surface(context, new_surface).map(Surface::Software)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                device.context_surface_framebuffer_object(context)
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                device.context_surface_framebuffer_object(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn context_surface_size(&self, context: &Context) -> Result<Size2D<i32>, Error> {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                device.context_surface_size(context)
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                device.context_surface_size(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn context_surface_id(&self, context: &Context) -> Result<SurfaceID, Error> {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                device.context_surface_id(context)
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                device.context_surface_id(context)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        match (self, context_descriptor) {
            (&Device::Hardware(ref device),
             &ContextDescriptor::Hardware(ref context_descriptor)) => {
                device.context_descriptor_attributes(context_descriptor)
            }
            (&Device::Software(ref device),
             &ContextDescriptor::Software(ref context_descriptor)) => {
                device.context_descriptor_attributes(context_descriptor)
            }
            _ => panic!("Incompatible context!")
        }
    }

    pub fn get_proc_address(&self, context: &Context, symbol_name: &str) -> *const c_void {
        match (self, context) {
            (&Device::Hardware(ref device), &Context::Hardware(ref context)) => {
                device.get_proc_address(context, symbol_name)
            }
            (&Device::Software(ref device), &Context::Software(ref context)) => {
                device.get_proc_address(context, symbol_name)
            }
            _ => panic!("Incompatible context!"),
        }
    }
}
