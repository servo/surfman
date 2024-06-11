// surfman/surfman/src/platform/generic/egl/surface.rs
//
//! Functionality common to backends using EGL surfaces.

use super::context::CurrentContextGuard;
use super::device::EGL_FUNCTIONS;
use crate::egl;
use crate::egl::types::{EGLAttrib, EGLConfig, EGLContext, EGLDisplay, EGLSurface, EGLint};
use crate::gl;
use crate::gl::types::{GLint, GLuint};
use crate::gl_utils;
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::platform::generic::egl::ffi::EGLClientBuffer;
use crate::platform::generic::egl::ffi::EGLImageKHR;
use crate::platform::generic::egl::ffi::EGL_EXTENSION_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_GL_TEXTURE_2D_KHR;
use crate::platform::generic::egl::ffi::EGL_IMAGE_PRESERVED_KHR;
use crate::platform::generic::egl::ffi::EGL_NO_IMAGE_KHR;
use crate::renderbuffers::Renderbuffers;
use crate::Gl;
use crate::{ContextAttributes, ContextID, Error, SurfaceID, SurfaceInfo};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::c_void;
use std::ptr;

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct ExternalEGLSurfaces {
    pub(crate) draw: EGLSurface,
    pub(crate) read: EGLSurface,
}

pub struct EGLBackedSurface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) objects: EGLSurfaceObjects,
    pub(crate) destroyed: bool,
}

impl Debug for EGLBackedSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

unsafe impl Send for EGLBackedSurface {}

#[allow(dead_code)]
pub(crate) enum EGLSurfaceObjects {
    TextureImage {
        egl_image: EGLImageKHR,
        framebuffer_object: GLuint,
        texture_object: GLuint,
        renderbuffers: Renderbuffers,
    },
    Window {
        native_window: *const c_void,
        egl_surface: EGLSurface,
    },
}

pub(crate) struct EGLSurfaceTexture {
    pub(crate) surface: EGLBackedSurface,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

impl Debug for EGLSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl EGLBackedSurface {
    pub(crate) fn new_generic(
        gl: &Gl,
        egl_display: EGLDisplay,
        egl_context: EGLContext,
        context_id: ContextID,
        context_attributes: &ContextAttributes,
        size: &Size2D<i32>,
    ) -> EGLBackedSurface {
        let egl_image_attribs = [
            EGL_IMAGE_PRESERVED_KHR as EGLint,
            egl::FALSE as EGLint,
            egl::NONE as EGLint,
            0,
        ];

        unsafe {
            // Create our texture.
            let mut texture_object = 0;
            gl.GenTextures(1, &mut texture_object);
            // Save the current texture binding
            let mut old_texture_object = 0;
            gl.GetIntegerv(gl::TEXTURE_BINDING_2D, &mut old_texture_object);
            gl.BindTexture(gl::TEXTURE_2D, texture_object);
            // Unbind PIXEL_UNPACK_BUFFER, because if it is bound,
            // it can cause errors in glTexImage2D.
            // TODO: should this be inside a check for GL 2.0?
            let mut unpack_buffer = 0;
            gl.GetIntegerv(gl::PIXEL_UNPACK_BUFFER_BINDING, &mut unpack_buffer);
            if unpack_buffer != 0 {
                gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
            }
            gl.TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as GLint,
                size.width,
                size.height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                ptr::null(),
            );
            // Restore the old bindings
            gl.BindTexture(gl::TEXTURE_2D, old_texture_object as _);
            if unpack_buffer != 0 {
                gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, unpack_buffer as _);
            }

            // Create our image.
            let egl_client_buffer = texture_object as usize as EGLClientBuffer;
            let egl_image = (EGL_EXTENSION_FUNCTIONS.CreateImageKHR)(
                egl_display,
                egl_context,
                EGL_GL_TEXTURE_2D_KHR,
                egl_client_buffer,
                egl_image_attribs.as_ptr(),
            );

            // Create the framebuffer, and bind the texture to it.
            let framebuffer_object =
                gl_utils::create_and_bind_framebuffer(gl, gl::TEXTURE_2D, texture_object);

            // Bind renderbuffers as appropriate.
            let renderbuffers = Renderbuffers::new(gl, size, context_attributes);
            renderbuffers.bind_to_current_framebuffer(gl);

            debug_assert_eq!(
                gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                gl::FRAMEBUFFER_COMPLETE
            );

            EGLBackedSurface {
                context_id,
                size: *size,
                objects: EGLSurfaceObjects::TextureImage {
                    egl_image,
                    framebuffer_object,
                    texture_object,
                    renderbuffers,
                },
                destroyed: false,
            }
        }
    }

    pub(crate) fn new_window(
        egl_display: EGLDisplay,
        egl_config: EGLConfig,
        native_window: *mut c_void,
        context_id: ContextID,
        size: &Size2D<i32>,
    ) -> EGLBackedSurface {
        EGL_FUNCTIONS.with(|egl| unsafe {
            let window_surface_attribs = [egl::NONE as EGLAttrib];
            let egl_surface = egl.CreatePlatformWindowSurface(
                egl_display,
                egl_config,
                native_window,
                window_surface_attribs.as_ptr(),
            );
            assert_ne!(egl_surface, egl::NO_SURFACE);

            EGLBackedSurface {
                context_id,
                size: *size,
                objects: EGLSurfaceObjects::Window {
                    native_window,
                    egl_surface,
                },
                destroyed: false,
            }
        })
    }

    pub(crate) fn to_surface_texture(
        self,
        gl: &Gl,
    ) -> Result<EGLSurfaceTexture, (Error, EGLBackedSurface)> {
        unsafe {
            let egl_image = match self.objects {
                EGLSurfaceObjects::TextureImage { egl_image, .. } => egl_image,
                EGLSurfaceObjects::Window { .. } => return Err((Error::WidgetAttached, self)),
            };
            let texture_object = bind_egl_image_to_gl_texture(gl, egl_image);
            Ok(EGLSurfaceTexture {
                surface: self,
                texture_object,
                phantom: PhantomData,
            })
        }
    }

    pub(crate) fn destroy(
        &mut self,
        gl: &Gl,
        egl_display: EGLDisplay,
        context_id: ContextID,
    ) -> Result<Option<*const c_void>, Error> {
        if context_id != self.context_id {
            return Err(Error::IncompatibleSurface);
        }

        unsafe {
            match self.objects {
                EGLSurfaceObjects::TextureImage {
                    ref mut egl_image,
                    ref mut framebuffer_object,
                    ref mut texture_object,
                    ref mut renderbuffers,
                } => {
                    gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
                    gl.DeleteFramebuffers(1, framebuffer_object);
                    *framebuffer_object = 0;
                    renderbuffers.destroy(gl);

                    let result = (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(egl_display, *egl_image);
                    assert_ne!(result, egl::FALSE);
                    *egl_image = EGL_NO_IMAGE_KHR;

                    gl.DeleteTextures(1, texture_object);
                    *texture_object = 0;

                    self.destroyed = true;
                    Ok(None)
                }
                EGLSurfaceObjects::Window {
                    ref mut egl_surface,
                    ref mut native_window,
                } => {
                    EGL_FUNCTIONS.with(|egl| {
                        egl.DestroySurface(egl_display, *egl_surface);
                        *egl_surface = egl::NO_SURFACE;
                    });

                    self.destroyed = true;
                    Ok(Some(mem::replace(native_window, ptr::null())))
                }
            }
        }
    }

    // TODO(pcwalton): Damage regions.
    pub(crate) fn present(
        &self,
        egl_display: EGLDisplay,
        egl_context: EGLContext,
    ) -> Result<(), Error> {
        unsafe {
            match self.objects {
                EGLSurfaceObjects::Window { egl_surface, .. } => {
                    // The surface must be bound to the current context in EGL 1.4. Temporarily
                    // make this surface current to enforce this.
                    let _guard = CurrentContextGuard::new();

                    EGL_FUNCTIONS.with(|egl| {
                        egl.MakeCurrent(egl_display, egl_surface, egl_surface, egl_context);

                        let ok = egl.SwapBuffers(egl_display, egl_surface);
                        if ok != egl::FALSE {
                            Ok(())
                        } else {
                            Err(Error::PresentFailed(
                                egl.GetError().to_windowing_api_error(),
                            ))
                        }
                    })
                }
                EGLSurfaceObjects::TextureImage { .. } => Err(Error::NoWidgetAttached),
            }
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: match self.objects {
                EGLSurfaceObjects::TextureImage {
                    framebuffer_object, ..
                } => framebuffer_object,
                EGLSurfaceObjects::Window { .. } => 0,
            },
        }
    }

    pub(crate) fn id(&self) -> SurfaceID {
        match self.objects {
            EGLSurfaceObjects::TextureImage { egl_image, .. } => SurfaceID(egl_image as usize),
            EGLSurfaceObjects::Window { egl_surface, .. } => SurfaceID(egl_surface as usize),
        }
    }

    pub(crate) fn native_window(&self) -> Result<*const c_void, Error> {
        match self.objects {
            EGLSurfaceObjects::TextureImage { .. } => Err(Error::NoWidgetAttached),
            EGLSurfaceObjects::Window { native_window, .. } => Ok(native_window),
        }
    }

    pub(crate) fn unbind(&self, gl: &Gl, egl_display: EGLDisplay, egl_context: EGLContext) {
        // If we're current, we stay current, but with no surface attached.
        unsafe {
            EGL_FUNCTIONS.with(|egl| {
                if egl.GetCurrentContext() != egl_context {
                    return;
                }

                egl.MakeCurrent(egl_display, egl::NO_SURFACE, egl::NO_SURFACE, egl_context);

                match self.objects {
                    EGLSurfaceObjects::TextureImage {
                        framebuffer_object, ..
                    } => {
                        gl_utils::unbind_framebuffer_if_necessary(gl, framebuffer_object);
                    }
                    EGLSurfaceObjects::Window { .. } => {}
                }
            })
        }
    }

    pub(crate) fn egl_surfaces(&self) -> ExternalEGLSurfaces {
        match self.objects {
            EGLSurfaceObjects::Window { egl_surface, .. } => ExternalEGLSurfaces {
                draw: egl_surface,
                read: egl_surface,
            },
            EGLSurfaceObjects::TextureImage { .. } => ExternalEGLSurfaces::default(),
        }
    }
}

impl EGLSurfaceTexture {
    pub(crate) fn destroy(mut self, gl: &Gl) -> EGLBackedSurface {
        unsafe {
            gl.DeleteTextures(1, &self.texture_object);
            self.texture_object = 0;
            self.surface
        }
    }
}

impl Default for ExternalEGLSurfaces {
    #[inline]
    fn default() -> ExternalEGLSurfaces {
        ExternalEGLSurfaces {
            draw: egl::NO_SURFACE,
            read: egl::NO_SURFACE,
        }
    }
}

#[allow(dead_code)]
pub(crate) unsafe fn create_pbuffer_surface(
    egl_display: EGLDisplay,
    egl_config: EGLConfig,
    size: &Size2D<i32>,
) -> EGLSurface {
    let attributes = [
        egl::WIDTH as EGLint,
        size.width as EGLint,
        egl::HEIGHT as EGLint,
        size.height as EGLint,
        egl::TEXTURE_FORMAT as EGLint,
        egl::TEXTURE_RGBA as EGLint,
        egl::TEXTURE_TARGET as EGLint,
        egl::TEXTURE_2D as EGLint,
        egl::NONE as EGLint,
        0,
        0,
        0,
    ];

    EGL_FUNCTIONS.with(|egl| {
        let egl_surface = egl.CreatePbufferSurface(egl_display, egl_config, attributes.as_ptr());
        assert_ne!(egl_surface, egl::NO_SURFACE);
        egl_surface
    })
}

#[allow(dead_code)]
pub(crate) unsafe fn bind_egl_image_to_gl_texture(gl: &Gl, egl_image: EGLImageKHR) -> GLuint {
    let mut texture = 0;
    gl.GenTextures(1, &mut texture);
    debug_assert_ne!(texture, 0);

    let mut texture_binding = 0;
    gl.GetIntegerv(gl::TEXTURE_BINDING_2D, &mut texture_binding);

    // FIXME(pcwalton): Should this be `GL_TEXTURE_EXTERNAL_OES`?
    gl.BindTexture(gl::TEXTURE_2D, texture);
    (EGL_EXTENSION_FUNCTIONS.ImageTargetTexture2DOES)(gl::TEXTURE_2D, egl_image);
    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
    gl.TexParameteri(
        gl::TEXTURE_2D,
        gl::TEXTURE_WRAP_S,
        gl::CLAMP_TO_EDGE as GLint,
    );
    gl.TexParameteri(
        gl::TEXTURE_2D,
        gl::TEXTURE_WRAP_T,
        gl::CLAMP_TO_EDGE as GLint,
    );
    gl.BindTexture(gl::TEXTURE_2D, texture_binding as GLuint);

    debug_assert_eq!(gl.GetError(), gl::NO_ERROR);
    texture
}
