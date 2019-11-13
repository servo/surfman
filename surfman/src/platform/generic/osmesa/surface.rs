//! Wrapper for GL-renderable pixmaps on X11.

use crate::context::ContextID;
use crate::gl;
use crate::gl::types::{GLenum, GLint, GLuint, GLvoid};
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

use euclid::default::Size2D;
use std::cell::UnsafeCell;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

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
    pub(crate) pixels: UnsafeCell<Vec<u8>>,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
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
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

/// TODO(pcwalton): Allow rendering to native widgets.
pub enum NativeWidget {}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if (*self.pixels.get()).len() > 0 && self.size.width != 0 && self.size.height != 0 &&
                    !thread::panicking() {
                panic!("Should have destroyed the surface first with `destroy_surface()`!")
            }
        }
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
    pub fn create_surface(&mut self,
                          context: &Context,
                          _: SurfaceAccess,
                          surface_type: SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        let size = match surface_type {
            SurfaceType::Generic { ref size } => *size,
            SurfaceType::Widget { .. } => unreachable!(),
        };
        let pixels = UnsafeCell::new(vec![0; size.width as usize * size.height as usize * 4]);
        Ok(Surface { pixels, size, context_id: context.id })
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
    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, (Error, Surface)> {
        unsafe {
            drop(self.make_context_current(context));

            GL_FUNCTIONS.with(|gl| {
                // Create a texture.
                let mut gl_texture = 0;
                gl.GenTextures(1, &mut gl_texture);
                debug_assert_ne!(gl_texture, 0);
                gl.BindTexture(gl::TEXTURE_2D, gl_texture);

                // TODO(pcwalton): Can we avoid this copy somehow?
                gl.TexImage2D(gl::TEXTURE_2D,
                              0,
                              gl::RGBA8 as GLint,
                              surface.size.width,
                              surface.size.height,
                              0,
                              gl::RGBA,
                              gl::UNSIGNED_BYTE,
                              (*surface.pixels.get()).as_ptr() as *const GLvoid);

                // Initialize the texture, for convenience.
                gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
                gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

                gl.BindTexture(gl::TEXTURE_2D, 0);
                debug_assert_eq!(gl.GetError(), gl::NO_ERROR);

                Ok(SurfaceTexture { surface, gl_texture, phantom: PhantomData })
            })
        }
    }

    /// Destroys a surface.
    /// 
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    /// 
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(&self, _: &mut Context, surface: &mut Surface) -> Result<(), Error> {
        unsafe {
            (*surface.pixels.get()).clear();
        }
        Ok(())
    }

    /// Destroys a surface texture and returns the underlying surface.
    /// 
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    /// 
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, (Error, SurfaceTexture)> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.BindTexture(gl::TEXTURE_2D, 0);
                gl.DeleteTextures(1, &mut surface_texture.gl_texture);
                surface_texture.gl_texture = 0;
            }
        });

        Ok(surface_texture.surface)
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _surface: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
        // TODO(pcwalton)
        Err(Error::Unimplemented)
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    /// 
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    /// 
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    #[inline]
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        SurfaceInfo {
            size: surface.size,
            id: surface.id(),
            context_id: surface.context_id,
            framebuffer_object: 0,
        }
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    /// 
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.gl_texture
    }
}

impl Surface {
    #[inline]
    fn id(&self) -> SurfaceID {
        unsafe {
            SurfaceID((*self.pixels.get()).as_ptr() as usize)
        }
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
