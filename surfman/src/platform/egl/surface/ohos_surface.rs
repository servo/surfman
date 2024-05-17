// surfman/surfman/src/platform/egl/ohos_surface.rs
//
//! Surface management for OpenHarmony OS using EGL.

use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;

use euclid::default::Size2D;
use log::info;

use crate::egl;
use crate::egl::types::EGLSurface;
use crate::gl;
use crate::gl::types::{GLenum, GLuint};
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_EXTENSION_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_NO_IMAGE_KHR;
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType};

use super::super::context::{Context, GL_FUNCTIONS};
use super::super::device::Device;
use super::super::ohos_ffi::{
    NativeWindowOperation, OHNativeWindow, OH_NativeWindow_NativeWindowHandleOpt,
};
use super::{Surface, SurfaceTexture};

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

pub(crate) enum SurfaceObjects {
    Window { egl_surface: EGLSurface },
}

/// An OHOS native window.
pub struct NativeWidget {
    pub(crate) native_window: *mut OHNativeWindow,
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
        info!("Device create_surface with Context");
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { native_widget } => unsafe {
                self.create_window_surface(context, native_widget)
            },
        }
    }

    fn create_generic_surface(
        &mut self,
        _context: &Context,
        _size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        Err(Error::Unimplemented)
    }

    unsafe fn create_window_surface(
        &mut self,
        context: &Context,
        native_widget: NativeWidget,
    ) -> Result<Surface, Error> {
        let mut height: i32 = 0;
        let mut width: i32 = 0;
        // Safety: `OH_NativeWindow_NativeWindowHandleOpt` takes two output i32 pointers as
        // variable arguments when called with `GET_BUFFER_GEOMETRY`. See the OHNativeWindow
        // documentation for details:
        // https://gitee.com/openharmony/docs/blob/master/en/application-dev/reference/apis-arkgraphics2d/_native_window.md
        let result = unsafe {
            OH_NativeWindow_NativeWindowHandleOpt(
                native_widget.native_window,
                NativeWindowOperation::GET_BUFFER_GEOMETRY,
                &mut height as *mut i32,
                &mut width as *mut i32,
            )
        };
        assert_eq!(result, 0, "Failed to determine size of native window");
        EGL_FUNCTIONS.with(|egl| {
            let egl_surface = egl.CreateWindowSurface(
                self.egl_display,
                self.context_to_egl_config(context),
                native_widget.native_window as *const c_void,
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
    /// On OpenHarmony, currently only widget surfaces are implemented in surfman, so
    /// this method unconditionally returns the `WidgetAttached` error.
    pub fn create_surface_texture(
        &self,
        _context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        Err((Error::WidgetAttached, surface))
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
        error!("lock_surface_data not implemented yet for OHOS");
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
    /// Creates a native widget type from an `OHNativeWindow`.
    #[inline]
    pub unsafe fn from_native_window(native_window: *mut OHNativeWindow) -> NativeWidget {
        NativeWidget { native_window }
    }
}

impl Surface {
    pub(super) fn id(&self) -> SurfaceID {
        match self.objects {
            SurfaceObjects::Window { egl_surface } => SurfaceID(egl_surface as usize),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
