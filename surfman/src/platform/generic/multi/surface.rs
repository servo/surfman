// surfman/surfman/src/platform/generic/multi/surface.rs
//
//! A surface abstraction that allows the choice of backends dynamically.

use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::{Error, SurfaceAccess, SurfaceInfo, SurfaceType};
use super::context::Context;
use super::device::Device;

use std::fmt::{self, Debug, Formatter};

pub enum Surface<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(Def::Surface),
    Alternate(Alt::Surface),
}

pub enum SurfaceTexture<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(Def::SurfaceTexture),
    Alternate(Alt::SurfaceTexture),
}

pub enum NativeWidget<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(<Def::Connection as ConnectionInterface>::NativeWidget),
    Alternate(<Alt::Connection as ConnectionInterface>::NativeWidget),
}

impl<Def, Alt> Debug for Surface<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface")
    }
}

impl<Def, Alt> Debug for SurfaceTexture<Def, Alt> where Def: DeviceInterface,
                                                        Alt: DeviceInterface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture")
    }
}

impl<Def, Alt> Device<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    pub fn create_surface(&mut self,
                          context: &Context<Def, Alt>,
                          surface_access: SurfaceAccess,
                          surface_type: SurfaceType<NativeWidget<Def, Alt>>)
                          -> Result<Surface<Def, Alt>, Error> {
        match (&mut *self, context) {
            (&mut Device::Default(ref mut device), &Context::Default(ref context)) => {
                let surface_type = match surface_type {
                    SurfaceType::Generic { size } => SurfaceType::Generic { size },
                    SurfaceType::Widget {
                        native_widget: NativeWidget::Default(native_widget),
                    } => SurfaceType::Widget { native_widget },
                    SurfaceType::Widget { native_widget: _ } => {
                        return Err(Error::IncompatibleNativeWidget)
                    }
                };
                device.create_surface(context, surface_access, surface_type)
                      .map(Surface::Default)
            }
            (&mut Device::Alternate(ref mut device), &Context::Alternate(ref context)) => {
                let surface_type = match surface_type {
                    SurfaceType::Generic { size } => SurfaceType::Generic { size },
                    SurfaceType::Widget {
                        native_widget: NativeWidget::Alternate(native_widget),
                    } => SurfaceType::Widget { native_widget },
                    SurfaceType::Widget { native_widget: _ } => {
                        return Err(Error::IncompatibleNativeWidget)
                    }
                };
                device.create_surface(context, surface_access, surface_type)
                      .map(Surface::Alternate)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn create_surface_texture(&self,
                                  context: &mut Context<Def, Alt>,
                                  surface: Surface<Def, Alt>)
                                  -> Result<SurfaceTexture<Def, Alt>, (Error, Surface<Def, Alt>)> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match surface {
                    Surface::Default(surface) => {
                        match device.create_surface_texture(context, surface) {
                            Ok(surface_texture) => Ok(SurfaceTexture::Default(surface_texture)),
                            Err((err, surface)) => Err((err, Surface::Default(surface))),
                        }
                    }
                    _ => Err((Error::IncompatibleSurface, surface)),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface {
                    Surface::Alternate(surface) => {
                        match device.create_surface_texture(context, surface) {
                            Ok(surface_texture) => Ok(SurfaceTexture::Alternate(surface_texture)),
                            Err((err, surface)) => Err((err, Surface::Alternate(surface))),
                        }
                    }
                    _ => Err((Error::IncompatibleSurface, surface)),
                }
            }
            _ => Err((Error::IncompatibleContext, surface)),
        }
    }

    pub fn destroy_surface(&self, context: &mut Context<Def, Alt>, surface: &mut Surface<Def, Alt>)
                           -> Result<(), Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match *surface {
                    Surface::Default(ref mut surface) => device.destroy_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match *surface {
                    Surface::Alternate(ref mut surface) => device.destroy_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn destroy_surface_texture(&self,
                                   context: &mut Context<Def, Alt>,
                                   surface_texture: SurfaceTexture<Def, Alt>)
                                   -> Result<Surface<Def, Alt>,
                                             (Error, SurfaceTexture<Def, Alt>)> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match surface_texture {
                    SurfaceTexture::Default(surface_texture) => {
                        match device.destroy_surface_texture(context, surface_texture) {
                            Ok(surface) => Ok(Surface::Default(surface)),
                            Err((err, surface_texture)) => {
                                Err((err, SurfaceTexture::Default(surface_texture)))
                            }
                        }
                    }
                    _ => Err((Error::IncompatibleSurfaceTexture, surface_texture)),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface_texture {
                    SurfaceTexture::Alternate(surface_texture) => {
                        match device.destroy_surface_texture(context, surface_texture) {
                            Ok(surface) => Ok(Surface::Alternate(surface)),
                            Err((err, surface_texture)) => {
                                Err((err, SurfaceTexture::Alternate(surface_texture)))
                            }
                        }
                    }
                    _ => Err((Error::IncompatibleSurfaceTexture, surface_texture)),
                }
            }
            _ => Err((Error::IncompatibleContext, surface_texture)),
        }
    }

    pub fn present_surface(&self, context: &Context<Def, Alt>, surface: &mut Surface<Def, Alt>)
                           -> Result<(), Error> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => {
                match *surface {
                    Surface::Default(ref mut surface) => device.present_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => {
                match *surface {
                    Surface::Alternate(ref mut surface) => {
                        device.present_surface(context, surface)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        match *self {
            Device::Default(ref device) => device.surface_gl_texture_target(),
            Device::Alternate(ref device) => device.surface_gl_texture_target(),
        }
    }

    pub fn surface_info(&self, surface: &Surface<Def, Alt>) -> SurfaceInfo {
        match (self, surface) {
            (&Device::Default(ref device), Surface::Default(ref surface)) => {
                device.surface_info(surface)
            }
            (&Device::Alternate(ref device), Surface::Alternate(ref surface)) => {
                device.surface_info(surface)
            }
            _ => panic!("Incompatible context!"),
        }
    }

    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture<Def, Alt>) -> GLuint {
        match (self, surface_texture) {
            (&Device::Default(ref device), SurfaceTexture::Default(ref surface_texture)) => {
                device.surface_texture_object(surface_texture)
            }
            (&Device::Alternate(ref device), SurfaceTexture::Alternate(ref surface_texture)) => {
                device.surface_texture_object(surface_texture)
            }
            _ => panic!("Incompatible context!"),
        }
    }
}
