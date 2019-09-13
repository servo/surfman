//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use crate::egl::types::{EGLConfig, EGLSurface, EGLenum};
use crate::egl::{self, EGLint};
use crate::{ContextAttributeFlags, Error, FeatureFlags, GLInfo, SurfaceId};
use super::context::{self, Context, ContextDescriptor, ContextID};
use super::device::{Device, EGL_EXTENSION_FUNCTIONS};
use super::error::ToWindowingApiError;

use euclid::default::Size2D;
use gl;
use gl::types::{GLenum, GLint, GLuint};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use winapi::shared::dxgi::IDXGIKeyedMutex;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winnt::HANDLE;
use wio::com::ComPtr;

const BYTES_PER_PIXEL: i32 = 4;

const EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE: EGLenum = 0x3200;

pub struct Surface {
    pub(crate) share_handle: HANDLE,
    pub(crate) keyed_mutex: ComPtr<IDXGIKeyedMutex>,
    pub(crate) egl_surface: EGLSurface,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) context_descriptor: ContextDescriptor,
}

#[derive(Debug)]
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) local_egl_surface: EGLSurface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.egl_surface != egl::NO_SURFACE && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        unsafe {
            let attributes = [
                egl::WIDTH as EGLint,           size.width as EGLint,
                egl::HEIGHT as EGLint,          size.height as EGLint,
                egl::TEXTURE_FORMAT as EGLint,  egl::TEXTURE_RGBA as EGLint,
                egl::TEXTURE_TARGET as EGLint,  egl::TEXTURE_2D as EGLint,
                egl::NONE as EGLint,            0,
                0,                              0,
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

            // `mozangle` builds ANGLE with keyed mutexes for sharing. Use the
            // `EGL_ANGLE_keyed_mutex` extension to fetch the keyed mutex so we can grab it.
            let mut keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
            let result = (EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE)(
                self.native_display.egl_display(),
                egl_surface,
                EGL_DXGI_KEYED_MUTEX_ANGLE as EGLint,
                &mut keyed_mutex);
            assert!(!keyed_mutex.is_null());
            let keyed_mutex = ComPtr::from_raw(keyed_mutex);

            Ok(Surface {
                share_handle,
                keyed_mutex,
                size: *size,
                egl_surface,
                context_id: context.id,
                context_descriptor,
            })
        }
    }

    pub fn create_surface_texture(&self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, (Error, Surface)> {
        let local_egl_config = self.context_descriptor_to_egl_config(&surface.context_descriptor);
        println!("local egl config renderable type={:x}",
                 context::get_config_attr(self.native_display.egl_display(),
                                          local_egl_config,
                                          egl::RENDERABLE_TYPE as EGLint));

        unsafe {
            // First, create an EGL surface local to this thread.
            let pbuffer_attributes = [
                egl::WIDTH as EGLint,           surface.size.width,
                egl::HEIGHT as EGLint,          surface.size.height,
                egl::TEXTURE_FORMAT as EGLint,  egl::TEXTURE_RGBA as EGLint,
                egl::TEXTURE_TARGET as EGLint,  egl::TEXTURE_2D as EGLint,
                egl::NONE as EGLint,            0,
                0,                              0,
            ];
            println!("surface size={},{} share_handle={:x}",
                     surface.size.width,
                     surface.size.height,
                     surface.share_handle as usize);
            let local_egl_surface =
                egl::CreatePbufferFromClientBuffer(self.native_display.egl_display(),
                                                   EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                                                   surface.share_handle,
                                                   local_egl_config,
                                                   pbuffer_attributes.as_ptr());
            if local_egl_surface == egl::NO_SURFACE {
                let windowing_api_error = egl::GetError().to_windowing_api_error();
                return Err((Error::SurfaceImportFailed(windowing_api_error), surface));
            }

            // Then bind that surface to the texture.
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl::BindTexture(gl::TEXTURE_2D, texture);
            if egl::BindTexImage(self.native_display.egl_display(),
                                 local_egl_surface,
                                 egl::BACK_BUFFER as GLint) == egl::FALSE {
                let windowing_api_error = egl::GetError().to_windowing_api_error();
                return Err((Error::SurfaceTextureCreationFailed(windowing_api_error), surface));
            }

            // Initialize the texture, for convenience.
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

            gl::BindTexture(gl::TEXTURE_2D, 0);
            debug_assert_eq!(gl::GetError(), gl::NO_ERROR);

            Ok(SurfaceTexture {
                surface,
                local_egl_surface,
                gl_texture: texture,
                phantom: PhantomData,
            })
        }
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        self.make_context_not_current(context)?;
        unsafe {
            egl::DestroySurface(self.native_display.egl_display(), surface.egl_surface);
            surface.egl_surface = egl::NO_SURFACE;
        }

        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        unsafe {
            gl::DeleteTextures(1, &surface_texture.gl_texture);
            surface_texture.gl_texture = 0;

            egl::DestroySurface(self.native_display.egl_display(),
                                surface_texture.local_egl_surface);
        }

        Ok(surface_texture.surface)
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }


    #[inline]
    pub fn id(&self) -> SurfaceId {
        SurfaceId(self.share_handle as usize)
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
