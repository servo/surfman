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
use crate::{ContextID, Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use wayland_sys::client::wl_proxy;
use wayland_sys::egl::{WAYLAND_EGL_HANDLE, wl_egl_window};

// FIXME(pcwalton): Is this right, or should it be `TEXTURE_EXTERNAL_OES`?
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
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) wayland_objects: WaylandObjects,
    pub(crate) destroyed: bool,
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

/// A wrapper for a Wayland surface, with associated size.
#[derive(Clone)]
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
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { native_widget } => {
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
        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(err) => return Err((err, surface)),
        };

        unsafe {
            GL_FUNCTIONS.with(|gl| {
                let egl_image = match surface.wayland_objects {
                    WaylandObjects::TextureImage { egl_image, .. } => egl_image,
                    WaylandObjects::Window { .. } => return Err((Error::WidgetAttached, surface)),
                };
                let texture_object = surface::bind_egl_image_to_gl_texture(gl, egl_image);
                Ok(SurfaceTexture { surface, texture_object, phantom: PhantomData })
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
    pub fn destroy_surface(&self, context: &mut Context, surface: &mut Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
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

    /// Destroys a surface texture and returns the underlying surface.
    /// 
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    /// 
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(&self,
                                   context: &mut Context,
                                   mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, (Error, SurfaceTexture)> {
        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(err) => return Err((err, surface_texture)),
        };

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;
                Ok(surface_texture.surface)
            }
        })
    }

    /// Displays the contents of a widget surface on screen.
    /// 
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    /// 
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        // TODO(pcwalton): Damage regions.
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

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
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
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        SurfaceInfo {
            size: surface.size,
            id: surface.id(),
            context_id: surface.context_id,
            framebuffer_object: match surface.wayland_objects {
                WaylandObjects::TextureImage { framebuffer_object, .. } => framebuffer_object,
                WaylandObjects::Window { .. } => 0,
            },
        }
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    /// 
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.texture_object
    }
}

impl Surface {
    fn id(&self) -> SurfaceID {
        match self.wayland_objects {
            WaylandObjects::TextureImage { egl_image, .. } => SurfaceID(egl_image as usize),
            WaylandObjects::Window { egl_surface, .. } => SurfaceID(egl_surface as usize),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
