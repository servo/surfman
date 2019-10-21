//! Wrapper for GL-renderable pixmaps on X11.

use crate::context::ContextID;
use crate::gl;
use crate::gl::types::{GLenum, GLint, GLuint, GLvoid};
use crate::{Error, SurfaceID};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

use euclid::default::Size2D;
use std::cell::UnsafeCell;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

pub struct Surface {
    pub(crate) pixels: UnsafeCell<Vec<u8>>,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

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

impl Device {
    pub fn create_surface(&mut self, context: &Context, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        let pixels = UnsafeCell::new(vec![0; size.width as usize * size.height as usize * 4]);
        Ok(Surface { pixels, size: *size, context_id: context.id })
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
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

    pub fn destroy_surface(&self, _: &mut Context, surface: Surface) -> Result<(), Error> {
        unsafe {
            (*surface.pixels.get()).clear();
        }
        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.BindTexture(gl::TEXTURE_2D, 0);
                gl.DeleteTextures(1, &mut surface_texture.gl_texture);
                surface_texture.gl_texture = 0;
            }
        });

        Ok(surface_texture.surface)
    }

    #[inline]
    pub fn lock_surface_data<'s>(&self, surface: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
        Err(Error::Unimplemented)
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        unsafe {
            SurfaceID((*self.pixels.get()).as_ptr() as usize)
        }
    }

    #[inline]
    pub fn context_id(&self) -> ContextID {
        self.context_id
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }
}

pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
