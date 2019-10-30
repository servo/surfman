// surfman/surfman/src/platform/unix/wayland/surface.rs
//
//! A surface implementation using Wayland surfaces backed by TextureImage.

use crate::egl::types::{EGLSurface, EGLint};
use crate::egl;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl;
use crate::gl_utils;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::{EGLClientBuffer, EGLImageKHR};
use crate::platform::generic::egl::ffi::{EGL_EXTENSION_FUNCTIONS, EGL_GL_TEXTURE_2D_KHR};
use crate::platform::generic::egl::ffi::{EGL_IMAGE_PRESERVED_KHR, EGL_NO_IMAGE_KHR};
use crate::platform::generic::egl::surface;
use crate::renderbuffers::Renderbuffers;
use crate::{ContextID, Error, SurfaceAccess, SurfaceID, SurfaceType};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

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
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

pub struct NativeWidget {
    pub(crate) wayland_surface: *mut wl_proxy,
    pub(crate) size: Size2D<i32>,
}

pub(crate) enum WaylandObjects {
    TextureImage {
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

        let egl_image_attribs = [
            EGL_IMAGE_PRESERVED_KHR as EGLint,  egl::FALSE as EGLint,
            egl::NONE as EGLint,                0,
        ];

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Create our texture.
                let mut texture_object = 0;
                gl.GenTextures(1, &mut texture_object);
                gl.BindTexture(SURFACE_GL_TEXTURE_TARGET, texture_object);
                gl.TexImage2D(SURFACE_GL_TEXTURE_TARGET,
                              0,
                              gl::RGBA as GLint,
                              size.width,
                              size.height,
                              0,
                              gl::RGBA,
                              gl::UNSIGNED_BYTE,
                              ptr::null());

                // Create our image.
                let egl_display = self.native_connection.egl_display();
                let egl_image =
                    (EGL_EXTENSION_FUNCTIONS.CreateImageKHR)(egl_display,
                                                             context.native_context.egl_context(),
                                                             EGL_GL_TEXTURE_2D_KHR,
                                                             texture_object as EGLClientBuffer,
                                                             egl_image_attribs.as_ptr());

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
                    wayland_objects: WaylandObjects::TextureImage {
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

        EGL_FUNCTIONS.with(|egl| {
            let egl_surface = egl.CreateWindowSurface(self.native_connection.egl_display(),
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
        })
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        unsafe {
            GL_FUNCTIONS.with(|gl| {
                let _guard = self.temporarily_make_context_current(context)?;
                let egl_image = match surface.wayland_objects {
                    WaylandObjects::TextureImage { egl_image, .. } => egl_image,
                    WaylandObjects::Window { .. } => return Err(Error::WidgetAttached),
                };
                let texture_object = surface::bind_egl_image_to_gl_texture(gl, egl_image);
                Ok(SurfaceTexture { surface, texture_object, phantom: PhantomData })
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
                WaylandObjects::TextureImage {
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

                        let egl_display = self.native_connection.egl_display();
                        let result = (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(egl_display,
                                                                               *egl_image);
                        assert_ne!(result, egl::FALSE);
                        *egl_image = EGL_NO_IMAGE_KHR;

                        gl.DeleteTextures(1, texture_object);
                        *texture_object = 0;
                    });
                }
                WaylandObjects::Window { ref mut egl_surface, ref mut egl_window } => {
                    EGL_FUNCTIONS.with(|egl| {
                        egl.DestroySurface(self.native_connection.egl_display(), *egl_surface);
                        *egl_surface = egl::NO_SURFACE;
                    });

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
                Ok(surface_texture.surface)
            }
        })
    }

    // TODO(pcwalton): Damage regions.
    pub(crate) fn present_surface_without_context(&self, surface: &mut Surface)
                                                  -> Result<(), Error> {
        unsafe {
            match surface.wayland_objects {
                WaylandObjects::Window { egl_surface, .. } => {
                    EGL_FUNCTIONS.with(|egl| {
                        egl.SwapBuffers(self.native_connection.egl_display(), egl_surface);
                    });
                    Ok(())
                }
                WaylandObjects::TextureImage { .. } => Err(Error::NoWidgetAttached),
            }
        }
    }

    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface)
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
            WaylandObjects::TextureImage { egl_image, .. } => SurfaceID(egl_image as usize),
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
    pub fn from_winit_window(window: &Window) -> NativeWidget {
        // The window's DPI factor is 1.0 when nothing has been rendered to it yet. So use the DPI
        // factor of the primary monitor instead, since that's where the window will presumably go
        // when actually displayed. (The user might move it somewhere else later, of course.)
        //
        // FIXME(pcwalton): Is it true that the window will go the primary monitor first?
        let hidpi_factor = window.get_primary_monitor().get_hidpi_factor();
        let window_size = window.get_inner_size().unwrap().to_physical(hidpi_factor);

        NativeWidget {
            wayland_surface: window.get_wayland_surface().unwrap() as *mut wl_proxy,
            size: Size2D::new(window_size.width as i32, window_size.height as i32),
        }
    }
}
