//! A wrapper around X11 `EGLDisplay`s.

use super::connection::{Connection, NativeConnectionWrapper};
use super::context::{Context, ContextDescriptor, NativeContext};
use super::surface::Surface;
use crate::base::egl::{
    context::{self, CurrentContextGuard, EGLBackedContext},
    surface::EGLBackedSurface,
};
use crate::context::ContextID;
use crate::egl::types::EGLint;
use crate::gl;
pub use crate::mesa_surfaceless::device::Adapter;
use crate::x11::surface::{NativeWidget, SurfaceDataGuard, SurfaceTexture};
use crate::{egl, ContextAttributes, Error, GLApi, Gl, SurfaceAccess, SurfaceInfo, SurfaceType};
use euclid::default::Size2D;
use glow::Texture;
use std::os::raw::c_void;
use std::sync::Arc;
use x11_dl::xlib::Window;

// FIXME(pcwalton): Is this right, or should it be `TEXTURE_EXTERNAL_OES`?
const SURFACE_GL_TEXTURE_TARGET: u32 = gl::TEXTURE_2D;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) native_connection: Arc<NativeConnectionWrapper>,
    pub(crate) adapter: Adapter,
}

/// Wraps an adapter.
///
/// On X11, devices and adapters are essentially identical types.
#[derive(Clone)]
pub struct NativeDevice {
    /// The hardware adapter corresponding to this device.
    pub adapter: Adapter,
}

impl Device {
    #[inline]
    pub(crate) fn new(connection: &Connection, adapter: &Adapter) -> Result<Device, Error> {
        Ok(Device {
            native_connection: connection.native_connection.clone(),
            adapter: (*adapter).clone(),
        })
    }

    /// Returns the native device corresponding to this device.
    ///
    /// This method is essentially an alias for the `adapter()` method on Wayland, since there is
    /// no explicit concept of a device on this backend.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice {
            adapter: self.adapter(),
        }
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection {
            native_connection: self.native_connection.clone(),
        }
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    #[inline]
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        // Set environment variables as appropriate.
        self.adapter.set_environment_variables();

        unsafe {
            ContextDescriptor::new(
                self.native_connection.egl_display,
                attributes,
                &[
                    egl::SURFACE_TYPE as EGLint,
                    egl::WINDOW_BIT as EGLint,
                    egl::RENDERABLE_TYPE as EGLint,
                    egl::OPENGL_BIT as EGLint,
                ],
            )
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    #[inline]
    pub fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        unsafe {
            let context = EGLBackedContext::new(
                self.native_connection.egl_display,
                descriptor,
                share_with.map(|ctx| &ctx.0),
                self.gl_api(),
            )?;
            context.make_current(self.native_connection.egl_display)?;
            Ok(Context(
                context,
                Gl::from_loader_function(context::get_proc_address),
            ))
        }
    }

    /// Wraps an `EGLContext` in a native context and returns it.
    ///
    /// The context is not retained, as there is no way to do this in the EGL API. Therefore,
    /// it is the caller's responsibility to ensure that the returned `Context` object remains
    /// alive as long as the `EGLContext` is.
    #[inline]
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext,
    ) -> Result<Context, Error> {
        Ok(Context(
            EGLBackedContext::from_native_context(native_context),
            Gl::from_loader_function(context::get_proc_address),
        ))
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if let Ok(Some(mut surface)) = self.unbind_surface_from_context(context) {
            self.destroy_surface(context, &mut surface)?;
        }

        unsafe {
            context.0.destroy(self.native_connection.egl_display);
            Ok(())
        }
    }

    /// Given a context, returns its underlying EGL context and attached surfaces.
    #[inline]
    pub fn native_context(&self, context: &Context) -> NativeContext {
        context.0.native_context()
    }

    /// Returns the descriptor that this context was created with.
    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            ContextDescriptor::from_egl_context(
                &context.1,
                self.native_connection.egl_display,
                context.0.egl_context,
            )
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    #[inline]
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe { context.0.make_current(self.native_connection.egl_display) }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    #[inline]
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe { context::make_no_context_current(self.native_connection.egl_display) }
    }

    #[inline]
    pub(crate) fn temporarily_make_context_current(
        &self,
        context: &Context,
    ) -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    /// Returns the attributes that the context descriptor was created with.
    #[inline]
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        unsafe { context_descriptor.attributes(self.native_connection.egl_display) }
    }

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        context::get_proc_address(symbol_name)
    }

    /// Attaches a surface to a context for rendering.
    ///
    /// This function takes ownership of the surface. The surface must have been created with this
    /// context, or an `IncompatibleSurface` error is returned.
    ///
    /// If this function is called with a surface already bound, a `SurfaceAlreadyBound` error is
    /// returned. To avoid this error, first unbind the existing surface with
    /// `unbind_surface_from_context`.
    ///
    /// If an error is returned, the surface is returned alongside it.
    #[inline]
    pub fn bind_surface_to_context(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<(), (Error, Surface)> {
        unsafe {
            context
                .0
                .bind_surface(self.native_connection.egl_display, surface.0)
                .map_err(|(err, surface)| (err, Surface(surface)))
        }
    }

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context,
    ) -> Result<Option<Surface>, Error> {
        unsafe {
            context
                .0
                .unbind_surface(&context.1, self.native_connection.egl_display)
                .map(|maybe_surface| maybe_surface.map(Surface))
        }
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        context
            .0
            .present_bound_surface(self.native_connection.egl_display)
    }

    /// If the currently bound surface is a widget surface, resize it,
    pub fn resize_bound_surface(
        &self,
        context: &mut Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        context.0.resize_bound_surface(size)
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.0.id
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    #[inline]
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        context.0.surface_info()
    }

    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &self,
        context: &Context,
        _: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { native_widget } => unsafe {
                self.create_window_surface(context, native_widget.window)
            },
        }
    }

    fn create_generic_surface(
        &self,
        context: &Context,
        size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        let _guard = self.temporarily_make_context_current(context)?;
        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);

        Ok(Surface(EGLBackedSurface::new_generic(
            &context.1,
            self.native_connection.egl_display,
            context.0.egl_context,
            context.0.id,
            &context_attributes,
            size,
        )))
    }

    unsafe fn create_window_surface(
        &self,
        context: &Context,
        mut x11_window: Window,
    ) -> Result<Surface, Error> {
        let egl_config_id = context::get_context_attr(
            self.native_connection.egl_display,
            context.0.egl_context,
            egl::CONFIG_ID as EGLint,
        );
        let egl_config =
            context::egl_config_from_id(self.native_connection.egl_display, egl_config_id);

        let display_guard = self.native_connection.lock_display();
        let (mut root_window, mut x, mut y, mut width, mut height) = (0, 0, 0, 0, 0);
        let (mut border_width, mut depth) = (0, 0);
        (self.native_connection.xlib.XGetGeometry)(
            display_guard.display(),
            x11_window,
            &mut root_window,
            &mut x,
            &mut y,
            &mut width,
            &mut height,
            &mut border_width,
            &mut depth,
        );
        let size = Size2D::new(width as i32, height as i32);

        Ok(Surface(EGLBackedSurface::new_window(
            self.native_connection.egl_display,
            egl_config,
            &mut x11_window as *mut Window as *mut c_void,
            context.0.id,
            &size,
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

        match surface.0.to_surface_texture(&context.1) {
            Ok(surface_texture) => Ok(SurfaceTexture(surface_texture)),
            Err((err, surface)) => Err((err, Surface(surface))),
        }
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
        let egl_display = self.native_connection.egl_display;
        surface.0.destroy(&context.1, egl_display, context.0.id)?;
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
        surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        match self.temporarily_make_context_current(context) {
            Ok(_guard) => Ok(Surface(surface_texture.0.destroy(&context.1))),
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
        surface.0.resize(size);
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
    pub fn surface_gl_texture_target(&self) -> u32 {
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
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> Option<Texture> {
        surface_texture.0.texture_object
    }
}
