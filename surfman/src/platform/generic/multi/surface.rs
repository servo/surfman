// surfman/surfman/src/platform/generic/multi/surface.rs
//
//! A surface abstraction that allows the choice of backends dynamically.

use super::context::Context;
use super::device::Device;
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use crate::gl::types::{GLenum, GLuint};
use crate::{Error, SurfaceAccess, SurfaceInfo, SurfaceType};
use euclid::default::Size2D;

use std::fmt::{self, Debug, Formatter};

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
///
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
///
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
pub enum Surface<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default surface type.
    Default(Def::Surface),
    /// The alternate surface type.
    Alternate(Alt::Surface),
}

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
pub enum SurfaceTexture<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default surface texture type.
    Default(Def::SurfaceTexture),
    /// The alternate surface texture type.
    Alternate(Alt::SurfaceTexture),
}

/// A native widget/window type that can dynamically switch between backends.
pub enum NativeWidget<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// The default native widget type.
    Default(<Def::Connection as ConnectionInterface>::NativeWidget),
    /// The alternate native widget type.
    Alternate(<Alt::Connection as ConnectionInterface>::NativeWidget),
}

impl<Def, Alt> Debug for Surface<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface")
    }
}

impl<Def, Alt> Debug for SurfaceTexture<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture")
    }
}

impl<Def, Alt> Device<Def, Alt>
where
    Def: DeviceInterface,
    Alt: DeviceInterface,
{
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &mut self,
        context: &Context<Def, Alt>,
        surface_access: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget<Def, Alt>>,
    ) -> Result<Surface<Def, Alt>, Error> {
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
                device
                    .create_surface(context, surface_access, surface_type)
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
                device
                    .create_surface(context, surface_access, surface_type)
                    .map(Surface::Alternate)
            }
            _ => Err(Error::IncompatibleContext),
        }
    }

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
    pub fn create_surface_texture(
        &self,
        context: &mut Context<Def, Alt>,
        surface: Surface<Def, Alt>,
    ) -> Result<SurfaceTexture<Def, Alt>, (Error, Surface<Def, Alt>)> {
        match (self, &mut *context) {
            (&Device::Default(ref device), &mut Context::Default(ref mut context)) => match surface
            {
                Surface::Default(surface) => {
                    match device.create_surface_texture(context, surface) {
                        Ok(surface_texture) => Ok(SurfaceTexture::Default(surface_texture)),
                        Err((err, surface)) => Err((err, Surface::Default(surface))),
                    }
                }
                _ => Err((Error::IncompatibleSurface, surface)),
            },
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

    /// Destroys a surface.
    ///
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(
        &self,
        context: &mut Context<Def, Alt>,
        surface: &mut Surface<Def, Alt>,
    ) -> Result<(), Error> {
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

    /// Destroys a surface texture and returns the underlying surface.
    ///
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    ///
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context<Def, Alt>,
        surface_texture: SurfaceTexture<Def, Alt>,
    ) -> Result<Surface<Def, Alt>, (Error, SurfaceTexture<Def, Alt>)> {
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

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(
        &self,
        context: &Context<Def, Alt>,
        surface: &mut Surface<Def, Alt>,
    ) -> Result<(), Error> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => match *surface {
                Surface::Default(ref mut surface) => device.present_surface(context, surface),
                _ => Err(Error::IncompatibleSurface),
            },
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => match *surface {
                Surface::Alternate(ref mut surface) => device.present_surface(context, surface),
                _ => Err(Error::IncompatibleSurface),
            },
            _ => Err(Error::IncompatibleContext),
        }
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        context: &Context<Def, Alt>,
        surface: &mut Surface<Def, Alt>,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        match (self, context) {
            (&Device::Default(ref device), &Context::Default(ref context)) => match *surface {
                Surface::Default(ref mut surface) => device.resize_surface(context, surface, size),
                _ => Err(Error::IncompatibleSurface),
            },
            (&Device::Alternate(ref device), &Context::Alternate(ref context)) => match *surface {
                Surface::Alternate(ref mut surface) => {
                    device.resize_surface(context, surface, size)
                }
                _ => Err(Error::IncompatibleSurface),
            },
            _ => Err(Error::IncompatibleContext),
        }
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        match *self {
            Device::Default(ref device) => device.surface_gl_texture_target(),
            Device::Alternate(ref device) => device.surface_gl_texture_target(),
        }
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
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

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
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
