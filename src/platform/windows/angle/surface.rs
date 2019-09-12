//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use crate::egl::types::{EGLConfig, EGLSurface, EGLenum};
use crate::egl::{self, EGLint};
use crate::{ContextAttributeFlags, Error, FeatureFlags, GLInfo, SurfaceId};
use super::context::Context;
use super::device::{Device, EGL_EXTENSION_FUNCTIONS};

use euclid::default::Size2D;
use gl;
use gl::types::{GLenum, GLint, GLuint};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winnt::HANDLE;

const BYTES_PER_PIXEL: i32 = 4;

const EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE: EGLenum = 0x3200;

pub struct Surface {
    pub(crate) share_handle: HANDLE,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) egl_surface: EGLSurface,
}

#[derive(Debug)]
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:?})", self.data.descriptor)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.data.destroyed.load(Ordering::SeqCst) && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        let egl_config = self.context_descriptor_to_egl_config(context_descriptor);
        unsafe {
            let attributes = [
                egl::WIDTH as EGLint,  size.width as EGLint,
                egl::HEIGHT as EGLint, size.height as EGLint,
                egl::NONE as EGLint,   0,
                0,                     0,
            ];

            let egl_surface = egl::CreatePbufferSurface(self.native_display.egl_display(),
                                                        egl_config,
                                                        attributes.as_ptr());
            assert_ne!(egl_surface, egl::NO_SURFACE);

            let mut share_handle = INVALID_HANDLE_VALUE;
            let result = (EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE)(
                self.native_display.egl_display(),
                egl_surface,
                EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE as EGLint,
                &mut share_handle);
            assert_ne!(result, egl::FALSE);
            assert_ne!(share_handle, INVALID_HANDLE_VALUE);

            let surface = Surface {
                data: Arc::new(SurfaceData {
                    share_handle,
                    context_descriptor: (*context_descriptor).clone(),
                    destroyed: AtomicBool::new(false),
                }),
            };

            self.surface_bindings.push(SurfaceBinding {
                surface: surface.clone(),
                egl_surface,
                egl_config,
            });

            Ok(surface)
        }
    }

    pub fn create_surface_texture(&mut self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        unsafe {
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl::BindTexture(gl::TEXTURE_2D, texture);

            let egl_surface = self.lookup_surface(&surface).egl_surface;
            if egl::BindTexImage(self.native_display.egl_display(),
                                 egl_surface,
                                 egl::BACK_BUFFER as GLint) == egl::FALSE {
                panic!("Failed to bind EGL texture surface: {:x}!", egl::GetError())
            }

            // Initialize the texture, for convenience.
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

            gl::BindTexture(gl::TEXTURE_2D, 0);
            debug_assert_eq!(gl::GetError(), gl::NO_ERROR);

            Ok(SurfaceTexture { surface, gl_texture: texture, phantom: PhantomData })
        }
    }

    pub fn destroy_surface(&self, mut surface: Surface) -> Result<(), Error> {
        // TODO(pcwalton): GC dead surfaces occasionally.
        // TODO(pcwalton): Check for double free?
        surface.data.destroyed.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        unsafe {
            gl::DeleteTextures(1, &surface_texture.gl_texture);
            surface_texture.gl_texture = 0;
        }

        Ok(surface_texture.surface)
    }

    pub(crate) fn lookup_surface(&self, surface: &Surface) -> Option<EGLSurface> {
        for binding in &self.surface_bindings {
            if binding.surface.data.ptr_eq(&*surface.data) {
                return Some(binding.egl_surface);
            }
        }
        None
    }
}

impl Surface {
    #[inline]
    pub fn descriptor(&self) -> &SurfaceDescriptor {
        &self.data.descriptor
    }

    #[inline]
    pub fn id(&self) -> SurfaceId {
        SurfaceId(self.data.share_handle as usize)
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }

    #[inline]
    pub fn gl_texture_target() -> GLenum {
        gl::TEXTURE_2D
    }
}

pub(crate) enum Framebuffer {
    // No surface has been attached to the context.
    None,
    // The surface is externally-managed.
    External,
    // The context renders to a surface that we manage.
    Surface(Surface),
}
