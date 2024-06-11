// surfman/surfman/src/platform/unix/wayland/surface.rs
//
//! A surface implementation using Wayland surfaces backed by TextureImage.

use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;
use crate::gl;
use crate::gl::types::{GLenum, GLuint};
use crate::platform::generic::egl::context;
use crate::platform::generic::egl::surface::{EGLBackedSurface, EGLSurfaceTexture};
use crate::{Error, SurfaceAccess, SurfaceInfo, SurfaceType};

use euclid::default::Size2D;
use std::marker::PhantomData;
use std::os::raw::c_void;
use wayland_sys::client::wl_proxy;
use wayland_sys::egl::{wl_egl_window, WAYLAND_EGL_HANDLE};

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
#[derive(Debug)]
pub struct Surface(pub(crate) EGLBackedSurface);

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
#[derive(Debug)]
pub struct SurfaceTexture(pub(crate) EGLSurfaceTexture);

/// A wrapper for a Wayland surface, with associated size.
#[derive(Clone)]
pub struct NativeWidget {
    pub(crate) wayland_surface: *mut wl_proxy,
    pub(crate) size: Size2D<i32>,
}

unsafe impl Send for Surface {}

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
                self.create_window_surface(
                    context,
                    native_widget.wayland_surface,
                    &native_widget.size,
                )
            },
        }
    }

    fn create_generic_surface(
        &mut self,
        context: &Context,
        size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        let _guard = self.temporarily_make_context_current(context)?;
        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);
        GL_FUNCTIONS.with(|gl| {
            Ok(Surface(EGLBackedSurface::new_generic(
                gl,
                self.native_connection.egl_display,
                context.0.egl_context,
                context.0.id,
                &context_attributes,
                size,
            )))
        })
    }

    unsafe fn create_window_surface(
        &mut self,
        context: &Context,
        wayland_surface: *mut wl_proxy,
        size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        let egl_window =
            (WAYLAND_EGL_HANDLE.wl_egl_window_create)(wayland_surface, size.width, size.height);
        assert!(!egl_window.is_null());

        let context_descriptor = self.context_descriptor(context);
        let egl_config = context::egl_config_from_id(
            self.native_connection.egl_display,
            context_descriptor.egl_config_id,
        );

        Ok(Surface(EGLBackedSurface::new_window(
            self.native_connection.egl_display,
            egl_config,
            egl_window as *mut c_void,
            context.0.id,
            size,
        )))
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
        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(err) => return Err((err, surface)),
        };

        GL_FUNCTIONS.with(|gl| match surface.0.to_surface_texture(gl) {
            Ok(surface_texture) => Ok(SurfaceTexture(surface_texture)),
            Err((err, surface)) => Err((err, Surface(surface))),
        })
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
        GL_FUNCTIONS.with(|gl| {
            let egl_display = self.native_connection.egl_display;
            if let Some(wayland_egl_window) = surface.0.destroy(gl, egl_display, context.0.id)? {
                unsafe {
                    let wayland_egl_window = wayland_egl_window as *mut wl_egl_window;
                    (WAYLAND_EGL_HANDLE.wl_egl_window_destroy)(wayland_egl_window);
                }
            }
            Ok(())
        })
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
        surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        match self.temporarily_make_context_current(context) {
            Ok(_guard) => GL_FUNCTIONS.with(|gl| Ok(Surface(surface_texture.0.destroy(gl)))),
            Err(err) => Err((err, surface_texture)),
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
        surface
            .0
            .present(self.native_connection.egl_display, context.0.egl_context)
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        _context: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        let wayland_egl_window = surface.0.native_window()? as *mut c_void as *mut wl_egl_window;
        unsafe {
            (WAYLAND_EGL_HANDLE.wl_egl_window_resize)(
                wayland_egl_window,
                size.width,
                size.height,
                0,
                0,
            )
        };
        surface.0.size = size;
        Ok(())
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface) -> Result<SurfaceDataGuard<'s>, Error> {
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
        surface.0.info()
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.0.texture_object
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
