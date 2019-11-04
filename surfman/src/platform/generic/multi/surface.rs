// surfman/surfman/src/platform/generic/multi/surface.rs
//
//! A surface abstraction that allows the choice of backends dynamically.

use crate::device::Device as DeviceInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::surface::{Surface as SurfaceInterface, SurfaceTexture as SurfaceTextureInterface};
use crate::{ContextID, Error, SurfaceAccess, SurfaceID, SurfaceType};
use super::context::Context;
use super::device::Device;

use euclid::default::Size2D;
use std::marker::PhantomData;

#[derive(Debug)]
pub enum Surface<Def, Alt> where Def: DeviceInterface,
                                 Alt: DeviceInterface,
                                 Def::Surface: SurfaceInterface,
                                 Alt::Surface: SurfaceInterface {
    Default(Def::Surface),
    Alternate(Alt::Surface),
}

pub enum SurfaceTexture<Def, Alt> where Def: DeviceInterface,
                                        Alt: DeviceInterface,
                                        Def::SurfaceTexture: SurfaceTextureInterface,
                                        Alt::SurfaceTexture: SurfaceTextureInterface {
    Default(Def::SurfaceTexture),
    Alternate(Alt::SurfaceTexture),
}

pub enum NativeWidget<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(Def::NativeWidget),
    Alternate(Alt::NativeWidget),
}

impl<Def, Alt> Device<Def, Alt> where Def: DeviceInterface,
                                      Alt: DeviceInterface,
                                      Def::NativeWidget: Clone,
                                      Alt::NativeWidget: Clone,
                                      Def::Surface: SurfaceInterface,
                                      Alt::Surface: SurfaceInterface,
                                      Def::SurfaceTexture: SurfaceTextureInterface,
                                      Alt::SurfaceTexture: SurfaceTextureInterface {
    pub fn create_surface(&mut self,
                          context: &Context<Def, Alt>,
                          surface_access: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget<Def, Alt>>)
                          -> Result<Surface<Def, Alt>, Error> {
        match (&mut *self, context) {
            (&mut Device::Default(ref mut device), &Context::Default(ref context)) => {
                let surface_type = match *surface_type {
                    SurfaceType::Generic { size } => SurfaceType::Generic { size },
                    SurfaceType::Widget {
                        native_widget: NativeWidget::Default(ref native_widget),
                    } => SurfaceType::Widget { native_widget: (*native_widget).clone() },
                    SurfaceType::Widget { native_widget: _ } => {
                        return Err(Error::IncompatibleNativeWidget)
                    }
                };
                device.create_surface(context, surface_access, &surface_type)
                      .map(Surface::Default)
            }
            (&mut Device::Alternate(ref mut device), &Context::Alternate(ref context)) => {
                let surface_type = match *surface_type {
                    SurfaceType::Generic { size } => SurfaceType::Generic { size },
                    SurfaceType::Widget {
                        native_widget: NativeWidget::Alternate(ref native_widget),
                    } => SurfaceType::Widget { native_widget: (*native_widget).clone() },
                    SurfaceType::Widget { native_widget: _ } => {
                        return Err(Error::IncompatibleNativeWidget)
                    }
                };
                device.create_surface(context, surface_access, &surface_type)
                      .map(Surface::Alternate)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn create_surface_texture(&self,
                                  context: &mut Context<Def, Alt>,
                                  surface: Surface<Def, Alt>)
                                  -> Result<SurfaceTexture<Def, Alt>, Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match surface {
                    Surface::Default(surface) => {
                        device.create_surface_texture(context, surface)
                              .map(SurfaceTexture::Default)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface {
                    Surface::Alternate(surface) => {
                        device.create_surface_texture(context, surface)
                              .map(SurfaceTexture::Alternate)
                    }
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn destroy_surface(&self, context: &mut Context<Def, Alt>, surface: Surface<Def, Alt>)
                           -> Result<(), Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match surface {
                    Surface::Default(surface) => device.destroy_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface {
                    Surface::Alternate(surface) => device.destroy_surface(context, surface),
                    _ => Err(Error::IncompatibleSurface),
                }
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

    pub fn destroy_surface_texture(&self,
                                   context: &mut Context<Def, Alt>,
                                   surface_texture: SurfaceTexture<Def, Alt>)
                                   -> Result<Surface<Def, Alt>, Error> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => {
                match surface_texture {
                    SurfaceTexture::Default(surface_texture) => {
                        device.destroy_surface_texture(context, surface_texture)
                              .map(Surface::Default)
                    }
                    _ => Err(Error::IncompatibleSurfaceTexture),
                }
            }
            (&Device::Alternate(ref device), &mut Context::Alternate(ref mut context)) => {
                match surface_texture {
                    SurfaceTexture::Alternate(surface_texture) => {
                        device.destroy_surface_texture(context, surface_texture)
                              .map(Surface::Alternate)
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
            Device::Default(ref device) => device.surface_gl_texture_target(),
            Device::Alternate(ref device) => device.surface_gl_texture_target(),
        }
    }
}

impl<Def, Alt> Surface<Def, Alt> where Def: DeviceInterface,
                                       Alt: DeviceInterface,
                                       Def::Surface: SurfaceInterface,
                                       Alt::Surface: SurfaceInterface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        match *self {
            Surface::Default(ref surface) => surface.size(),
            Surface::Alternate(ref surface) => surface.size(),
        }
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        match *self {
            Surface::Default(ref surface) => surface.id(),
            Surface::Alternate(ref surface) => surface.id(),
        }
    }

    #[inline]
    pub fn context_id(&self) -> ContextID {
        match *self {
            Surface::Default(ref surface) => surface.context_id(),
            Surface::Alternate(ref surface) => surface.context_id(),
        }
    }

    #[inline]
    pub fn framebuffer_object(&self) -> GLuint {
        match *self {
            Surface::Default(ref surface) => surface.framebuffer_object(),
            Surface::Alternate(ref surface) => surface.framebuffer_object(),
        }
    }
}

impl<Def, Alt> SurfaceTexture<Def, Alt> where Def: DeviceInterface,
                                              Alt: DeviceInterface,
                                              Def::SurfaceTexture: SurfaceTextureInterface,
                                              Alt::SurfaceTexture: SurfaceTextureInterface {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        match *self {
            SurfaceTexture::Default(ref surface_texture) => surface_texture.gl_texture(),
            SurfaceTexture::Alternate(ref surface_texture) => surface_texture.gl_texture(),
        }
    }
}

impl<Def, Alt> SurfaceInterface for Surface<Def, Alt> where Def: DeviceInterface,
                                                            Alt: DeviceInterface,
                                                            Def::Surface: SurfaceInterface,
                                                            Alt::Surface: SurfaceInterface {
    #[inline]
    fn size(&self) -> Size2D<i32> {
        Surface::size(self)
    }

    #[inline]
    fn id(&self) -> SurfaceID {
        Surface::id(self)
    }
    
    #[inline]
    fn context_id(&self) -> ContextID {
        Surface::context_id(self)
    }
    
    #[inline]
    fn framebuffer_object(&self) -> GLuint {
        Surface::framebuffer_object(self)
    }
}

impl<Def, Alt> SurfaceTextureInterface for SurfaceTexture<Def, Alt>
                                       where Def: DeviceInterface,
                                             Alt: DeviceInterface,
                                             Def::SurfaceTexture: SurfaceTextureInterface,
                                             Alt::SurfaceTexture: SurfaceTextureInterface {
    #[inline]
    fn gl_texture(&self) -> GLuint {
        SurfaceTexture::gl_texture(self)
    }
}
