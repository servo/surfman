// surfman/surfman/src/platform/macos/cgl/surface.rs
//
//! Surface management for macOS.

use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;
use crate::context::ContextID;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl_utils;
use crate::platform::macos::system::surface::Surface as SystemSurface;
use crate::renderbuffers::Renderbuffers;
use crate::{gl, Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType, WindowingApiError};

use core_foundation::base::TCFType;
use euclid::default::Size2D;
use io_surface::{self, IOSurface};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;

pub use crate::platform::macos::system::surface::{NativeSurface, NativeWidget};

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_RECTANGLE;

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
pub struct Surface {
    pub(crate) system_surface: SystemSurface,
    pub(crate) context_id: ContextID,
    pub(crate) framebuffer_object: GLuint,
    pub(crate) texture_object: GLuint,
    pub(crate) renderbuffers: Renderbuffers,
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
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &mut self,
        context: &Context,
        access: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        let mut system_surface = self.0.create_surface(access, surface_type)?;
        self.0.set_surface_flipped(&mut system_surface, true);

        let _guard = self.temporarily_make_context_current(context);
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let mut texture_object =
                    self.bind_to_gl_texture(&system_surface.io_surface, &system_surface.size);

                let mut framebuffer_object = 0;
                gl.GenFramebuffers(1, &mut framebuffer_object);
                let _guard = self.temporarily_bind_framebuffer(framebuffer_object);

                gl.FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    SURFACE_GL_TEXTURE_TARGET,
                    texture_object,
                    0,
                );

                let context_descriptor = self.context_descriptor(context);
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);

                let mut renderbuffers =
                    Renderbuffers::new(gl, &system_surface.size, &context_attributes);
                renderbuffers.bind_to_current_framebuffer(gl);

                if gl.GetError() != gl::NO_ERROR
                    || gl.CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE
                {
                    // On macos, surface creation can fail silently (e.g. due to OOM) and AFAICT
                    // the way to tell that it has failed is to look at the framebuffer status
                    // while the surface is attached.
                    renderbuffers.destroy(gl);
                    if framebuffer_object != 0 {
                        gl.DeleteFramebuffers(1, &mut framebuffer_object);
                    }
                    if texture_object != 0 {
                        gl.DeleteTextures(1, &mut texture_object);
                    }
                    let _ = self.0.destroy_surface(&mut system_surface);
                    // TODO: convert the GL error into a surfman error?
                    return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
                }

                Ok(Surface {
                    system_surface,
                    context_id: context.id,
                    framebuffer_object,
                    texture_object,
                    renderbuffers,
                })
            }
        })
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
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        if surface.system_surface.view_info.is_some() {
            return Err((Error::WidgetAttached, surface));
        }

        let _guard = self.temporarily_make_context_current(context).unwrap();

        let texture_object = self.bind_to_gl_texture(
            &surface.system_surface.io_surface,
            &surface.system_surface.size,
        );
        Ok(SurfaceTexture {
            surface,
            texture_object,
            phantom: PhantomData,
        })
    }

    fn bind_to_gl_texture(&self, io_surface: &IOSurface, size: &Size2D<i32>) -> GLuint {
        GL_FUNCTIONS.with(|gl| unsafe {
            let mut texture = 0;
            gl.GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl.BindTexture(gl::TEXTURE_RECTANGLE, texture);
            io_surface.bind_to_gl_texture(size.width, size.height, true);

            gl.TexParameteri(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_MAG_FILTER,
                gl::NEAREST as GLint,
            );
            gl.TexParameteri(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_MIN_FILTER,
                gl::NEAREST as GLint,
            );
            gl.TexParameteri(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            gl.TexParameteri(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );

            gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);

            debug_assert_eq!(gl.GetError(), gl::NO_ERROR);

            texture
        })
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
        context: &mut Context,
        surface: &mut Surface,
    ) -> Result<(), Error> {
        GL_FUNCTIONS.with(|gl| {
            if context.id != surface.context_id {
                return Err(Error::IncompatibleSurface);
            }

            unsafe {
                gl_utils::destroy_framebuffer(gl, surface.framebuffer_object);
                surface.framebuffer_object = 0;

                surface.renderbuffers.destroy(gl);
                gl.DeleteTextures(1, &surface.texture_object);
                surface.texture_object = 0;
            }

            self.0.destroy_surface(&mut surface.system_surface)
        })
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
        _: &mut Context,
        mut surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;
            }

            Ok(surface_texture.surface)
        })
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.texture_object
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        self.0.present_surface(&mut surface.system_surface)?;

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let size = surface.system_surface.size;
                gl.BindTexture(gl::TEXTURE_RECTANGLE, surface.texture_object);
                surface
                    .system_surface
                    .io_surface
                    .bind_to_gl_texture(size.width, size.height, true);
                gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);
            }

            Ok(())
        })
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        let _guard = self.temporarily_make_context_current(context);
        let _guard = self.temporarily_bind_framebuffer(surface.framebuffer_object);

        self.0.resize_surface(&mut surface.system_surface, size)?;

        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Recreate the GL texture and bind it to the FBO
                let texture_object =
                    self.bind_to_gl_texture(&surface.system_surface.io_surface, &size);
                gl.FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    SURFACE_GL_TEXTURE_TARGET,
                    texture_object,
                    0,
                );

                // Recreate the GL renderbuffers and bind them to the FBO
                let renderbuffers = Renderbuffers::new(gl, &size, &context_attributes);
                renderbuffers.bind_to_current_framebuffer(gl);

                gl.DeleteTextures(1, &surface.texture_object);
                surface.renderbuffers.destroy(gl);

                surface.texture_object = texture_object;
                surface.renderbuffers = renderbuffers;

                debug_assert_eq!(
                    (gl.GetError(), gl.CheckFramebufferStatus(gl::FRAMEBUFFER)),
                    (gl::NO_ERROR, gl::FRAMEBUFFER_COMPLETE),
                );
            }

            Ok(())
        })
    }

    fn temporarily_bind_framebuffer(&self, new_framebuffer: GLuint) -> FramebufferGuard {
        GL_FUNCTIONS.with(|gl| unsafe {
            let (mut current_draw_framebuffer, mut current_read_framebuffer) = (0, 0);
            gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut current_draw_framebuffer);
            gl.GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut current_read_framebuffer);
            gl.BindFramebuffer(gl::FRAMEBUFFER, new_framebuffer);
            FramebufferGuard {
                draw: current_draw_framebuffer as GLuint,
                read: current_read_framebuffer as GLuint,
            }
        })
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    #[inline]
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        let system_surface_info = self.0.surface_info(&surface.system_surface);
        SurfaceInfo {
            size: system_surface_info.size,
            id: system_surface_info.id,
            context_id: surface.context_id,
            framebuffer_object: surface.framebuffer_object,
        }
    }

    /// Returns the native `IOSurface` corresponding to this surface.
    ///
    /// The reference count is increased on the `IOSurface` before returning.
    #[inline]
    pub fn native_surface(&self, surface: &Surface) -> NativeSurface {
        self.0.native_surface(&surface.system_surface)
    }
}

impl Surface {
    #[inline]
    fn id(&self) -> SurfaceID {
        SurfaceID(self.system_surface.io_surface.as_concrete_TypeRef() as usize)
    }
}

#[must_use]
struct FramebufferGuard {
    draw: GLuint,
    read: GLuint,
}

impl Drop for FramebufferGuard {
    fn drop(&mut self) {
        GL_FUNCTIONS.with(|gl| unsafe {
            gl.BindFramebuffer(gl::READ_FRAMEBUFFER, self.read);
            gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.draw);
        })
    }
}
