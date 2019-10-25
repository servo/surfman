// surfman/surfman/src/platform/unix/wayland/surface.rs
//
//! A surface implementation using Wayland surfaces backed by GBM.

use crate::egl::types::{EGLClientBuffer, EGLImageKHR, EGLSurface, EGLint};
use crate::egl;
use crate::gl::types::{GLchar, GLenum, GLint, GLuint, GLvoid};
use crate::gl;
use crate::gl_utils;
use crate::platform::generic::egl::surface;
use crate::renderbuffers::Renderbuffers;
use crate::{ContextID, Error, SurfaceAccess, SurfaceID, SurfaceType, WindowingApiError};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;
use super::ffi::{EGL_DRM_BUFFER_FORMAT_ARGB32_MESA, EGL_DRM_BUFFER_FORMAT_MESA};
use super::ffi::{EGL_DRM_BUFFER_MESA, EGL_DRM_BUFFER_STRIDE_MESA};
use super::ffi::{EGL_DRM_BUFFER_USE_MESA, EGL_EXTENSION_FUNCTIONS};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use wayland_sys::client::wl_proxy;
use wayland_sys::egl::{WAYLAND_EGL_HANDLE, wl_egl_window};

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

// FIXME(pcwalton): Is this right, or should it be `TEXTURE_EXTERNAL_OES`?
const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

pub struct Surface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) wayland_objects: WaylandObjects,
    pub(crate) destroyed: bool,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) local_egl_image: EGLImageKHR,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

pub struct NativeWidget {
    pub(crate) wayland_surface: *mut wl_proxy,
    pub(crate) size: Size2D<i32>,
}

pub(crate) enum WaylandObjects {
    GBM {
        drm_image_name: EGLint,
        drm_image_stride: EGLint,
        egl_image: EGLImageKHR,
        framebuffer_object: GLuint,
        texture_object: GLuint,
        renderbuffers: Renderbuffers,
    },
    Window {
        egl_window: *mut wl_egl_window,
        egl_surface: EGLSurface,
    },
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Device {
    pub fn create_surface(&mut self,
                          context: &Context,
                          _: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        match *surface_type {
            SurfaceType::Generic { ref size } => self.create_generic_surface(context, size),
            SurfaceType::Widget { ref native_widget } => {
                unsafe {
                    self.create_window_surface(context,
                                               native_widget.wayland_surface,
                                               &native_widget.size)
                }
            }
        }
    }

    fn create_generic_surface(&mut self, context: &Context, size: &Size2D<i32>)
                              -> Result<Surface, Error> {
        let _guard = self.temporarily_make_context_current(context)?;

        let egl_drm_image_attribs = [
            egl::WIDTH as EGLint,                   size.width,
            egl::HEIGHT as EGLint,                  size.height,
            EGL_DRM_BUFFER_FORMAT_MESA as EGLint,   EGL_DRM_BUFFER_FORMAT_ARGB32_MESA as EGLint,
            EGL_DRM_BUFFER_USE_MESA as EGLint,      0,
            egl::NONE as EGLint,                    0,
        ];

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Create our EGL image.
                let egl_display = self.native_display.egl_display();
                let egl_image = (EGL_EXTENSION_FUNCTIONS.CreateDRMImageMESA)(
                    egl_display,
                    egl_drm_image_attribs.as_ptr());
                if egl_image == egl::NO_IMAGE_KHR {
                    return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
                }

                // Extract the DRM name and stride for that image.
                let (mut drm_image_name, mut drm_image_stride) = (0, 0);
                let ok = (EGL_EXTENSION_FUNCTIONS.ExportDRMImageMESA)(egl_display,
                                                                      egl_image,
                                                                      &mut drm_image_name,
                                                                      ptr::null_mut(),
                                                                      &mut drm_image_stride);
                assert_ne!(ok, egl::FALSE);

                // Initialize and bind the image to the texture.
                let texture_object = surface::bind_egl_image_to_gl_texture(gl, egl_image);

                // Create the framebuffer, and bind the texture to it.
                let framebuffer_object =
                    gl_utils::create_and_bind_framebuffer(gl,
                                                          SURFACE_GL_TEXTURE_TARGET,
                                                          texture_object);

                // Bind renderbuffers as appropriate.
                let context_descriptor = self.context_descriptor(context);
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);
                let renderbuffers = Renderbuffers::new(gl, size, &context_attributes);
                renderbuffers.bind_to_current_framebuffer(gl);

                debug_assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                                 gl::FRAMEBUFFER_COMPLETE);

                Ok(Surface {
                    size: *size,
                    context_id: context.id,
                    wayland_objects: WaylandObjects::GBM {
                        drm_image_name,
                        drm_image_stride,
                        egl_image,
                        framebuffer_object,
                        texture_object,
                        renderbuffers,
                    },
                    destroyed: false,
                })
            }
        })
    }

    unsafe fn create_window_surface(&mut self,
                                    context: &Context,
                                    wayland_surface: *mut wl_proxy,
                                    size: &Size2D<i32>)
                                    -> Result<Surface, Error> {
        let egl_window = (WAYLAND_EGL_HANDLE.wl_egl_window_create)(wayland_surface,
                                                                   size.width,
                                                                   size.height);
        assert!(!egl_window.is_null());

        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        let egl_surface = egl::CreateWindowSurface(self.native_display.egl_display(),
                                                   egl_config,
                                                   egl_window as *const c_void,
                                                   ptr::null());
        assert_ne!(egl_surface, egl::NO_SURFACE);

        Ok(Surface {
            context_id: context.id,
            size: *size,
            wayland_objects: WaylandObjects::Window { egl_window, egl_surface },
            destroyed: false,
        })
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        let (drm_image_name, drm_image_stride) = match surface.wayland_objects {
            WaylandObjects::Window { .. } => return Err(Error::WidgetAttached),
            WaylandObjects::GBM { drm_image_name, drm_image_stride, .. } => {
                (drm_image_name, drm_image_stride)
            }
        };

        let local_egl_image_attribs = [
            egl::WIDTH as EGLint,                   surface.size.width,
            egl::HEIGHT as EGLint,                  surface.size.height,
            EGL_DRM_BUFFER_FORMAT_MESA as EGLint,   EGL_DRM_BUFFER_FORMAT_ARGB32_MESA as EGLint,
            EGL_DRM_BUFFER_STRIDE_MESA as EGLint,   drm_image_stride,
            egl::IMAGE_PRESERVED_KHR as EGLint,     egl::FALSE as EGLint,
            egl::NONE as EGLint,                    0,
        ];

        unsafe {
            GL_FUNCTIONS.with(|gl| {
                let _guard = self.temporarily_make_context_current(context)?;

                let local_egl_image =
                    egl::CreateImageKHR(self.native_display.egl_display(),
                                        context.native_context.egl_context(),
                                        EGL_DRM_BUFFER_MESA,
                                        drm_image_name as EGLClientBuffer,
                                        local_egl_image_attribs.as_ptr());

                let texture_object = surface::bind_egl_image_to_gl_texture(gl, local_egl_image);

                Ok(SurfaceTexture {
                    surface,
                    local_egl_image,
                    texture_object,
                    phantom: PhantomData,
                })
            })
        }
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // Leak the surface, and return an error.
            surface.destroyed = true;
            return Err(Error::IncompatibleSurface);
        }

        unsafe {
            match surface.wayland_objects {
                WaylandObjects::GBM {
                    ref mut drm_image_name,
                    drm_image_stride: _,
                    ref mut egl_image,
                    ref mut framebuffer_object,
                    ref mut texture_object,
                    ref mut renderbuffers,
                } => {
                    GL_FUNCTIONS.with(|gl| {
                        gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
                        gl.DeleteFramebuffers(1, framebuffer_object);
                        *framebuffer_object = 0;
                        renderbuffers.destroy(gl);

                        gl.DeleteTextures(1, texture_object);
                        *texture_object = 0;

                        let result = egl::DestroyImageKHR(self.native_display.egl_display(),
                                                          *egl_image);
                        assert_ne!(result, egl::FALSE);
                        *egl_image = egl::NO_IMAGE_KHR;

                        *drm_image_name = 0;
                    });
                }
                WaylandObjects::Window { ref mut egl_surface, ref mut egl_window } => {
                    egl::DestroySurface(self.native_display.egl_display(), *egl_surface);
                    *egl_surface = egl::NO_SURFACE;

                    (WAYLAND_EGL_HANDLE.wl_egl_window_destroy)(*egl_window);
                    *egl_window = ptr::null_mut();
                }
            }
        }

        surface.destroyed = true;
        Ok(())
    }

    pub fn destroy_surface_texture(&self,
                                   context: &mut Context,
                                   mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        let _guard = self.temporarily_make_context_current(context)?;
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;

                let result = egl::DestroyImageKHR(self.native_display.egl_display(),
                                                  surface_texture.local_egl_image);
                assert_ne!(result, egl::FALSE);
                surface_texture.local_egl_image = egl::NO_IMAGE_KHR;
            }

            Ok(surface_texture.surface)
        })
    }

    // TODO(pcwalton): Damage regions.
    pub(crate) fn present_surface_without_context(&self, surface: &mut Surface)
                                                  -> Result<(), Error> {
        unsafe {
            match surface.wayland_objects {
                WaylandObjects::Window { egl_surface, .. } => {
                    egl::SwapBuffers(self.native_display.egl_display(), egl_surface);
                    Ok(())
                }
                WaylandObjects::GBM { .. } => Err(Error::NoWidgetAttached),
            }
        }
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

    pub fn id(&self) -> SurfaceID {
        match self.wayland_objects {
            WaylandObjects::GBM { egl_image, .. } => SurfaceID(egl_image as usize),
            WaylandObjects::Window { egl_surface, .. } => SurfaceID(egl_surface as usize),
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
        self.texture_object
    }
}

pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}

impl NativeWidget {
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn from_winit_window(window: &Window) -> NativeWidget {
        unsafe {
            let hidpi_factor = window.get_hidpi_factor();
            let window_size = window.get_inner_size().unwrap().to_physical(hidpi_factor);
            NativeWidget {
                wayland_surface: window.get_wayland_surface().unwrap() as *mut wl_proxy,
                size: Size2D::new(window_size.width as i32, window_size.height as i32),
            }
        }
    }
}
