//! A surface abstraction that can switch between hardware and software rendering.

use crate::gl::types::{GLenum, GLuint};
use crate::platform::default::surface::Surface as HWSurface;
use crate::platform::default::surface::SurfaceTexture as HWSurfaceTexture;
use crate::platform::generic::osmesa::surface::Surface as OSMesaSurface;
use crate::platform::generic::osmesa::surface::SurfaceTexture as OSMesaSurfaceTexture;
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceType};
use super::context::Context;
use super::device::Device;

use euclid::default::Size2D;
use std::marker::PhantomData;

pub use crate::platform::default::surface::NativeWidget;

#[derive(Debug)]
pub enum Surface {
    Hardware(HWSurface),
    Software(OSMesaSurface),
}

pub enum SurfaceTexture {
    Hardware(HWSurfaceTexture),
    Software(OSMesaSurfaceTexture),
}

impl Device {
    pub fn create_surface(&mut self,
                          context: &Context,
                          surface_access: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        match (&mut *self, context) {
            (&mut Device::Hardware(ref mut device), &Context::Hardware(ref context)) => {
                device.create_surface(context, surface_access, surface_type).map(Surface::Hardware)
            }
            (&mut Device::Software(ref mut device), &Context::Software(ref context)) => {
                let surface_type = match *surface_type {
                    SurfaceType::Generic { size } => SurfaceType::Generic { size },
                    SurfaceType::Widget { .. } => {
                        // TODO(pcwalton): Allow rendering to native widgets using the universal
                        // backend.
                        return Err(Error::Unimplemented)
                    }
                };
                device.create_surface(context, surface_access, &surface_type)
                      .map(Surface::Software)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        match (self, &mut *context) {
            (&Device::Hardware(ref device), &mut Context::Hardware(ref mut context)) => {
                match surface {
                    Surface::Hardware(surface) => {
                        device.create_surface_texture(context, surface)
                              .map(SurfaceTexture::Hardware)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Software(ref device), &mut Context::Software(ref mut context)) => {
                match surface {
                    Surface::Software(surface) => {
                        device.create_surface_texture(context, surface)
                              .map(SurfaceTexture::Software)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn destroy_surface(&self, context: &mut Context, surface: Surface) -> Result<(), Error> {
        match (self, &mut *context) {
            (&Device::Hardware(ref device), &mut Context::Hardware(ref mut context)) => {
                match surface {
                    Surface::Hardware(surface) => device.destroy_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Software(ref device), &mut Context::Software(ref mut context)) => {
                match surface {
                    Surface::Software(surface) => device.destroy_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn destroy_surface_texture(&self, context: &mut Context, surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        match (self, &mut *context) {
            (&Device::Hardware(ref device), &mut Context::Hardware(ref mut context)) => {
                match surface_texture {
                    SurfaceTexture::Hardware(surface_texture) => {
                        device.destroy_surface_texture(context, surface_texture)
                              .map(Surface::Hardware)
                    }
                    _ => Err(Error::IncompatibleSurfaceTexture),
                }
            }
            (&Device::Software(ref device), &mut Context::Software(ref mut context)) => {
                match surface_texture {
                    SurfaceTexture::Software(surface_texture) => {
                        device.destroy_surface_texture(context, surface_texture)
                              .map(Surface::Software)
                    }
                    _ => Err(Error::IncompatibleSurfaceTexture),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        match *self {
            Device::Hardware(ref device) => device.surface_gl_texture_target(),
            Device::Software(ref device) => device.surface_gl_texture_target(),
        }
    }

    // TODO(pcwalton)
    #[inline]
    pub fn lock_surface_data<'s>(&self, _surface: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
        Err(Error::Unimplemented)
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        match *self {
            Surface::Hardware(ref surface) => surface.size(),
            Surface::Software(ref surface) => surface.size(),
        }
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        match *self {
            Surface::Hardware(ref surface) => surface.id(),
            Surface::Software(ref surface) => surface.id(),
        }
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        match *self {
            SurfaceTexture::Hardware(ref surface_texture) => surface_texture.gl_texture(),
            SurfaceTexture::Software(ref surface_texture) => surface_texture.gl_texture(),
        }
    }
}

pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
