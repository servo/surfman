// surfman/surfman/src/platform/android/surface.rs
//
//! Surface management for Android using the `GraphicBuffer` class and EGL.

use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;
use super::ffi::{AHardwareBuffer, AHardwareBuffer_Desc, AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE};
use super::ffi::{AHardwareBuffer_allocate, AHardwareBuffer_release, ANativeWindow};
use super::ffi::{ANativeWindow_getHeight, ANativeWindow_getWidth};
use super::ffi::{AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM, AHARDWAREBUFFER_USAGE_CPU_READ_NEVER};
use super::ffi::{AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER, AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER};
use crate::context::ContextID;
use crate::egl;
use crate::egl::types::{EGLSurface, EGLint};
use crate::gl;
use crate::gl::types::{GLenum, GLuint};
use crate::gl_utils;
use crate::platform::generic;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGLImageKHR;
use crate::platform::generic::egl::ffi::EGL_EXTENSION_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_IMAGE_PRESERVED_KHR;
use crate::platform::generic::egl::ffi::EGL_NATIVE_BUFFER_ANDROID;
use crate::platform::generic::egl::ffi::EGL_NO_IMAGE_KHR;
use crate::renderbuffers::Renderbuffers;
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType, WindowingApiError};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use std::thread;

pub use crate::platform::generic::egl::context::ContextDescriptor;

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
    pub(crate) objects: SurfaceObjects,
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

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

/// An Android native window.
pub struct NativeWidget {
    pub(crate) native_window: *mut ANativeWindow,
}

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &mut self,
        context: &Context,
        _: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { native_widget } => unsafe {
                self.create_window_surface(context, native_widget.native_window)
            },
        }
    }

    fn create_generic_surface(
        &mut self,
        context: &Context,
        size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        let _guard = self.temporarily_make_context_current(context)?;

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Create a native hardware buffer.
                let hardware_buffer_desc = AHardwareBuffer_Desc {
                    format: AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
                    height: size.height as u32,
                    width: size.width as u32,
                    layers: 1,
                    rfu0: 0,
                    rfu1: 0,
                    stride: 10,
                    usage: AHARDWAREBUFFER_USAGE_CPU_READ_NEVER
                        | AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER
                        | AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER
                        | AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE,
                };
                let mut hardware_buffer = ptr::null_mut();
                let result = AHardwareBuffer_allocate(&hardware_buffer_desc, &mut hardware_buffer);
                if result != 0 {
                    return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
                }

                // Create an EGL image, and bind it to a texture.
                let egl_image = self.create_egl_image(context, hardware_buffer);

                // Initialize and bind the image to the texture.
                let texture_object =
                    generic::egl::surface::bind_egl_image_to_gl_texture(gl, egl_image);

                // Create the framebuffer, and bind the texture to it.
                let framebuffer_object = gl_utils::create_and_bind_framebuffer(
                    gl,
                    SURFACE_GL_TEXTURE_TARGET,
                    texture_object,
                );

                // Bind renderbuffers as appropriate.
                let context_descriptor = self.context_descriptor(context);
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);
                let renderbuffers = Renderbuffers::new(gl, size, &context_attributes);
                renderbuffers.bind_to_current_framebuffer(gl);

                debug_assert_eq!(
                    gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                    gl::FRAMEBUFFER_COMPLETE
                );

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

    unsafe fn create_window_surface(
        &mut self,
        context: &Context,
        native_window: *mut ANativeWindow,
    ) -> Result<Surface, Error> {
        let width = ANativeWindow_getWidth(native_window);
        let height = ANativeWindow_getHeight(native_window);

        EGL_FUNCTIONS.with(|egl| {
            let egl_surface = egl.CreateWindowSurface(
                self.egl_display,
                self.context_to_egl_config(context),
                native_window as *const c_void,
                ptr::null(),
            );
            assert_ne!(egl_surface, egl::NO_SURFACE);

            Ok(Surface {
                context_id: context.id,
                size: Size2D::new(width, height),
                objects: SurfaceObjects::Window { egl_surface },
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
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        unsafe {
            match surface.objects {
                SurfaceObjects::Window { .. } => return Err((Error::WidgetAttached, surface)),
                SurfaceObjects::HardwareBuffer {
                    hardware_buffer, ..
                } => GL_FUNCTIONS.with(|gl| {
                    let _guard = match self.temporarily_make_context_current(context) {
                        Ok(guard) => guard,
                        Err(err) => return Err((err, surface)),
                    };

                    let local_egl_image = self.create_egl_image(context, hardware_buffer);
                    let texture_object =
                        generic::egl::surface::bind_egl_image_to_gl_texture(gl, local_egl_image);
                    Ok(SurfaceTexture {
                        surface,
                        local_egl_image,
                        texture_object,
                        phantom: PhantomData,
                    })
                }),
            }
        }
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| unsafe {
            match surface.objects {
                SurfaceObjects::Window { egl_surface } => {
                    egl.SwapBuffers(self.egl_display, egl_surface);
                    Ok(())
                }
                SurfaceObjects::HardwareBuffer { .. } => Err(Error::NoWidgetAttached),
            }
        })
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        _context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        surface.size = size;
        Ok(())
    }

    #[allow(non_snake_case)]
    unsafe fn create_egl_image(
        &self,
        _: &Context,
        hardware_buffer: *mut AHardwareBuffer,
    ) -> EGLImageKHR {
        // Get the native client buffer.
        let eglGetNativeClientBufferANDROID =
            EGL_EXTENSION_FUNCTIONS.GetNativeClientBufferANDROID.expect(
                "Where's the `EGL_ANDROID_get_native_client_buffer` \
                                            extension?",
            );
        let client_buffer =
            eglGetNativeClientBufferANDROID(hardware_buffer as *const AHardwareBuffer as *const _);
        assert!(!client_buffer.is_null());

        // Create the EGL image.
        let egl_image_attributes = [
            EGL_IMAGE_PRESERVED_KHR as EGLint,
            egl::TRUE as EGLint,
            egl::NONE as EGLint,
            0,
        ];
        let egl_image = (EGL_EXTENSION_FUNCTIONS.CreateImageKHR)(
            self.egl_display,
            egl::NO_CONTEXT,
            EGL_NATIVE_BUFFER_ANDROID,
            client_buffer,
            egl_image_attributes.as_ptr(),
        );
        assert_ne!(egl_image, EGL_NO_IMAGE_KHR);
        egl_image
    }

    /// Destroys a surface.
    ///
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(
        &self,
        context: &mut Context,
        surface: &mut Surface,
    ) -> Result<(), Error> {
        if context.id != surface.context_id {
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

                        let egl_display = self.egl_display;
                        let result =
                            (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(egl_display, *egl_image);
                        assert_ne!(result, egl::FALSE);
                        *egl_image = EGL_NO_IMAGE_KHR;

                        AHardwareBuffer_release(*hardware_buffer);
                        *hardware_buffer = ptr::null_mut();
                    });
                }
                SurfaceObjects::Window {
                    ref mut egl_surface,
                } => EGL_FUNCTIONS.with(|egl| {
                    egl.DestroySurface(self.egl_display, *egl_surface);
                    *egl_surface = egl::NO_SURFACE;
                }),
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
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context,
        mut surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        let _guard = self.temporarily_make_context_current(context);
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;

                let egl_display = self.egl_display;
                let result = (EGL_EXTENSION_FUNCTIONS.DestroyImageKHR)(
                    egl_display,
                    surface_texture.local_egl_image,
                );
                assert_ne!(result, egl::FALSE);
                surface_texture.local_egl_image = EGL_NO_IMAGE_KHR;
            }

            Ok(surface_texture.surface)
        })
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface) -> Result<SurfaceDataGuard<'s>, Error> {
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
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        SurfaceInfo {
            size: surface.size,
            id: surface.id(),
            context_id: surface.context_id,
            framebuffer_object: match surface.objects {
                SurfaceObjects::HardwareBuffer {
                    framebuffer_object, ..
                } => framebuffer_object,
                SurfaceObjects::Window { .. } => 0,
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

impl NativeWidget {
    /// Creates a native widget type from an Android `NativeWindow`.
    #[inline]
    pub unsafe fn from_native_window(native_window: *mut ANativeWindow) -> NativeWidget {
        NativeWidget { native_window }
    }
}

impl Surface {
    fn id(&self) -> SurfaceID {
        match self.objects {
            SurfaceObjects::HardwareBuffer { egl_image, .. } => SurfaceID(egl_image as usize),
            SurfaceObjects::Window { egl_surface } => SurfaceID(egl_surface as usize),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
