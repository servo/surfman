// surfman/surfman/src/platform/unix/generic/context.rs
//
//! OpenGL rendering contexts on surfaceless Mesa.

use super::device::Device;
use super::surface::Surface;
use crate::context::ContextID;
use crate::egl;
use crate::egl::types::EGLint;
use crate::platform::generic::egl::context::{self, CurrentContextGuard, EGLBackedContext};
use crate::{ContextAttributes, Error, Gl, SurfaceInfo};

use std::os::raw::c_void;

pub use crate::platform::generic::egl::context::{ContextDescriptor, NativeContext};

thread_local! {
    #[doc(hidden)]
    pub static GL_FUNCTIONS: Gl = Gl::load_with(context::get_proc_address);
}

/// Represents an OpenGL rendering context.
///
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
///
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
///
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
///
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub struct Context(pub(crate) EGLBackedContext);

impl Device {
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
                    egl::PBUFFER_BIT as EGLint,
                    egl::RENDERABLE_TYPE as EGLint,
                    egl::OPENGL_BIT as EGLint,
                    egl::COLOR_BUFFER_TYPE as EGLint,
                    egl::RGB_BUFFER as EGLint,
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
        &mut self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        unsafe {
            EGLBackedContext::new(
                self.native_connection.egl_display,
                descriptor,
                share_with.map(|ctx| &ctx.0),
                self.gl_api(),
            )
            .map(Context)
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
        Ok(Context(EGLBackedContext::from_native_context(
            native_context,
        )))
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
        GL_FUNCTIONS.with(|gl| unsafe {
            ContextDescriptor::from_egl_context(
                gl,
                self.native_connection.egl_display,
                context.0.egl_context,
            )
        })
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
        GL_FUNCTIONS.with(|gl| unsafe {
            context
                .0
                .unbind_surface(gl, self.native_connection.egl_display)
                .map(|maybe_surface| maybe_surface.map(Surface))
        })
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
}
