// surfman/src/platform/android/surface.rs

//! Surface management for Android using the `GraphicBuffer` class and
//! EGL.

use crate::context::ContextID;
use crate::egl::types::{EGLClientBuffer, EGLImageKHR, EGLSurface, EGLenum, EGLint};
use crate::gl::Gl;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::renderbuffers::Renderbuffers;
use crate::{Error, SurfaceID, WindowingApiError, egl, gl};
use super::context::{Context, GL_FUNCTIONS};
use super::device::{Device, EGL_EXTENSION_FUNCTIONS};
use super::ffi::{AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM, AHARDWAREBUFFER_USAGE_CPU_READ_NEVER};
use super::ffi::{AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER, AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER};
use super::ffi::{AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE, AHardwareBuffer, AHardwareBuffer_Desc};
use super::ffi::{AHardwareBuffer_allocate, AHardwareBuffer_release, ANativeWindow};
use super::ffi::{ANativeWindow_getHeight, ANativeWindow_getWidth};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use std::thread;

// FIXME(pcwalton): Is this right, or should it be `TEXTURE_EXTERNAL_OES`?
const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

const EGL_NATIVE_BUFFER_ANDROID: EGLenum = 0x3140;

pub struct Surface {
    pub(crate) context_id: ContextID,
    pub(crate) size: Size2D<i32>,
    pub(crate) objects: SurfaceObjects,
    pub(crate) destroyed: bool,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) local_egl_image: EGLImageKHR,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

pub(crate) enum SurfaceObjects {
    HardwareBuffer {
        hardware_buffer: *mut AHardwareBuffer,
        egl_image: EGLImageKHR,
        framebuffer_object: GLuint,
        texture_object: GLuint,
        renderbuffers: Renderbuffers,
    },
    Window {
        egl_surface: EGLSurface,
    },
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

pub enum SurfaceType {
    Generic { size: Size2D<i32> },
    Widget { native_widget: NativeWidget },
}

pub struct NativeWidget {
    pub(crate) native_window: *mut ANativeWindow,
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, surface_type: &SurfaceType)
                          -> Result<Surface, Error> {
        match *surface_type {
            SurfaceType::Generic { ref size } => self.create_generic_surface(context, size),
            SurfaceType::Widget { ref native_widget } => {
                unsafe {
                    self.create_window_surface(context, native_widget.native_window)
                }
            }
        }
    }

    fn create_generic_surface(&mut self, context: &Context, size: &Size2D<i32>)
                              -> Result<Surface, Error> {
        error!("create_generic_surface() point a");
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Create a native hardware buffer.
                let mut hardware_buffer_desc = AHardwareBuffer_Desc {
                    format: AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
                    height: size.height as u32,
                    width: size.width as u32,
                    layers: 1,
                    rfu0: 0,
                    rfu1: 0,
                    stride: 0,
                    usage: AHARDWAREBUFFER_USAGE_CPU_READ_NEVER |
                        AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER |
                        AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER |
                        AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE,
                };
                let mut hardware_buffer = ptr::null_mut();
                error!("create_generic_surface() point b");
                let result = AHardwareBuffer_allocate(&hardware_buffer_desc, &mut hardware_buffer);
                error!("create_generic_surface() point c");
                if result != 0 {
                    return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
                }

                // Create an EGL image, and bind it to a texture.
                error!("create_generic_surface() point d");
                let egl_image = self.create_egl_image(context, hardware_buffer);

                // Initialize and bind the image to the texture.
                error!("create_generic_surface() point b");
                let texture_object = self.bind_to_gl_texture(egl_image);

                // Create the framebuffer, and bind the texture to it.
                error!("create_generic_surface() point e");
                let mut framebuffer_object = 0;
                gl.GenFramebuffers(1, &mut framebuffer_object);
                gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
                gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        SURFACE_GL_TEXTURE_TARGET,
                                        texture_object,
                                        0);

                error!("create_generic_surface() point f");
                let context_descriptor = self.context_descriptor(context);
                error!("create_generic_surface() point g");
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);
                error!("create_generic_surface() point h");

                let renderbuffers = Renderbuffers::new(gl, size, &context_attributes);
                error!("create_generic_surface() point i");
                renderbuffers.bind_to_current_framebuffer(gl);
                error!("create_generic_surface() point j");

                debug_assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                                 gl::FRAMEBUFFER_COMPLETE);
                error!("create_generic_surface() point k");

                Ok(Surface {
                    size: *size,
                    context_id: context.id,
                    objects: SurfaceObjects::HardwareBuffer {
                        hardware_buffer,
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
                                    native_window: *mut ANativeWindow)
                                    -> Result<Surface, Error> {
        let width = ANativeWindow_getWidth(native_window);
        let height = ANativeWindow_getHeight(native_window);

        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        let egl_surface = egl::CreateWindowSurface(self.native_display.egl_display(),
                                                   egl_config,
                                                   native_window as *const c_void,
                                                   ptr::null());
        assert_ne!(egl_surface, egl::NO_SURFACE);

        Ok(Surface {
            context_id: context.id,
            size: Size2D::new(width, height),
            objects: SurfaceObjects::Window { egl_surface },
            destroyed: false,
        })
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        unsafe {
            match surface.objects {
                SurfaceObjects::Window { .. } => return Err(Error::WidgetAttached),
                SurfaceObjects::HardwareBuffer { hardware_buffer, .. } => {
                    let local_egl_image = self.create_egl_image(context, hardware_buffer);
                    let texture_object = self.bind_to_gl_texture(local_egl_image);
                    Ok(SurfaceTexture {
                        surface,
                        local_egl_image,
                        texture_object,
                        phantom: PhantomData,
                    })
                }
            }
        }
    }

    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        self.present_surface_without_context(surface)
    }

    pub(crate) fn present_surface_without_context(&self, surface: &mut Surface)
                                                  -> Result<(), Error> {
        unsafe {
            match surface.objects {
                SurfaceObjects::Window { egl_surface } => {
                    egl::SwapBuffers(self.native_display.egl_display(), egl_surface);
                    Ok(())
                }
                SurfaceObjects::HardwareBuffer { .. } => Err(Error::NoWidgetAttached),
            }
        }
    }

    unsafe fn create_egl_image(&self, context: &Context, hardware_buffer: *mut AHardwareBuffer)
                               -> EGLImageKHR {
        // Get the native client buffer.
        let client_buffer =
            (EGL_EXTENSION_FUNCTIONS.GetNativeClientBufferANDROID)(hardware_buffer);
        assert!(!client_buffer.is_null());

        // Create the EGL image.
        let egl_image_attributes = [
            egl::IMAGE_PRESERVED_KHR as EGLint, egl::TRUE as EGLint,
            egl::NONE as EGLint,                0,
        ];
        let egl_image = egl::CreateImageKHR(self.native_display.egl_display(),
                                            context.native_context.egl_context(),
                                            EGL_NATIVE_BUFFER_ANDROID,
                                            client_buffer,
                                            egl_image_attributes.as_ptr());
        if egl_image == egl::NO_IMAGE_KHR {
            error!("*** failed to create EGL image: {:x}!", egl::GetError());
        }
        assert_ne!(egl_image, egl::NO_IMAGE_KHR);
        egl_image
    }

    unsafe fn bind_to_gl_texture(&self, egl_image: EGLImageKHR) -> GLuint {
        GL_FUNCTIONS.with(|gl| {
            let mut texture = 0;
            gl.GenTextures(1, &mut texture);
            debug_assert_ne!(texture, 0);

            gl.BindTexture(gl::TEXTURE_2D, texture);
            (EGL_EXTENSION_FUNCTIONS.ImageTargetTexture2DOES)(gl::TEXTURE_2D, egl_image);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
            gl.BindTexture(gl::TEXTURE_2D, 0);

            debug_assert_eq!(gl.GetError(), gl::NO_ERROR);
            texture
        })
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // Leak the surface, and return an error.
            surface.destroyed = true;
            return Err(Error::IncompatibleSurface);
        }

        unsafe {
            match surface.objects {
                SurfaceObjects::HardwareBuffer {
                    ref mut hardware_buffer,
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

                        AHardwareBuffer_release(*hardware_buffer);
                        *hardware_buffer = ptr::null_mut();
                    });
                }
                SurfaceObjects::Window { ref mut egl_surface } => {
                    egl::DestroySurface(self.native_display.egl_display(), *egl_surface);
                    *egl_surface = egl::NO_SURFACE;
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
        let _guard = self.temporarily_make_context_current(context);
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

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }
}

impl NativeWidget {
    #[inline]
    pub unsafe fn from_native_window(native_window: *mut ANativeWindow) -> NativeWidget {
        NativeWidget { native_window }
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    pub fn id(&self) -> SurfaceID {
        match self.objects {
            SurfaceObjects::HardwareBuffer { egl_image, .. } => SurfaceID(egl_image as usize),
            SurfaceObjects::Window { egl_surface } => SurfaceID(egl_surface as usize),
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
