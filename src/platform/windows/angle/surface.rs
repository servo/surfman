//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use crate::{ContextAttributeFlags, Error, FeatureFlags, GLInfo, SurfaceDescriptor, SurfaceId};
use super::context::Context;
use super::device::Device;
use super::surface::SurfaceBinding;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use gl;
use gl::types::{GLenum, GLint, GLuint};
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow};
use io_surface::{kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Clone)]
pub struct Surface {
    pub(crate) data: Arc<SurfaceData>,
}

pub(crate) struct SurfaceData {
    pub(crate) share_handle: HANDLE,
    pub(crate) descriptor: SurfaceDescriptor,
    pub(crate) destroyed: AtomicBool,
}

#[derive(Debug)]
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

pub(crate) struct SurfaceBinding {
    pub(crate) surface: Surface,
    pub(crate) egl_surface: EGLSurface,
    pub(crate) egl_config: EGLConfig,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:?})", self.descriptor)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface_from_descriptor(&mut self,
                                          context: &mut Context,
                                          descriptor: &SurfaceDescriptor)
                                          -> Result<Surface, Error> {
        unsafe {
            let egl_config = self.flavor_to_config(&descriptor.flavor);

            let attributes = [
                egl::WIDTH as EGLint,  descriptor.size.width as EGLint,
                egl::HEIGHT as EGLint, descriptor.size.height as EGLint,
                egl::NONE as EGLint,   0,
                0,                     0,
            ];

            let egl_surface = egl::CreatePbufferSurface(self.egl_display,
                                                        egl_config,
                                                        attributes.as_ptr());
            assert_ne!(egl_surface, egl::NO_SURFACE);

            let mut share_handle = INVALID_HANDLE_VALUE;
            let result =
                eglQuerySurfacePointerANGLE(self.egl_display,
                                            egl_surface,
                                            EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE as EGLint,
                                            &mut share_handle);
            assert_ne!(result, egl::FALSE);
            assert_ne!(share_handle, INVALID_HANDLE_VALUE);

            let surface = Surface {
                data: Arc::new(SurfaceData {
                    share_handle,
                    descriptor: *descriptor,
                    destroyed: AtomicBool::new(false),
                },
            };

            self.surfaces.push(SurfaceBinding {
                surface: surface.clone(),
                egl_surface,
                egl_config,
            });

            Ok(surface)
        }
    }

    pub fn create_surface_texture(&self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        unsafe {
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl::BindTexture(gl::TEXTURE_2D, texture);

            let egl_surface = self.get_angle_surface(&surface).egl_surface;
            if egl::BindTexImage(self.egl_display, egl_surface, egl::BACK_BUFFER as GLint) ==
                    egl::FALSE {
                panic!("Failed to bind EGL texture surface: {:x}!", egl::GetError())
            }

            // Low filtering to allow rendering
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

            // TODO(emilio): Check if these two are neccessary, probably not
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

            gl.BindTexture(gl::TEXTURE_2D, 0);
            debug_assert_eq!(gl::GetError(), gl::NO_ERROR);

            Ok(SurfaceTexture { surface, gl_texture: texture, phantom: PhantomData })
        }
    }

    pub fn destroy_surface(&self, _: &mut Context, mut surface: Surface) -> Result<(), Error> {
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
}

impl Surface {
    #[inline]
    pub fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
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

pub(crate) enum ColorSurface {
    // No surface has been attached to the context.
    None,
    // The surface is externally-managed.
    External,
    // The context renders to a DXGI surface that we manage.
    Managed(Surface),
}
