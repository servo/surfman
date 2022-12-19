// surfman/surfman/src/platform/android/context.rs
//
//! OpenGL rendering contexts.

use super::device::Device;
use super::surface::{Surface, SurfaceObjects};
use crate::context::{ContextID, CREATE_CONTEXT_MUTEX};
use crate::egl;
use crate::egl::types::{EGLConfig, EGLContext, EGLSurface, EGLint};
use crate::platform::generic::egl::context::{self, CurrentContextGuard};
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::platform::generic::egl::surface::ExternalEGLSurfaces;
use crate::surface::Framebuffer;
use crate::{ContextAttributes, Error, Gl, SurfaceInfo};

use std::mem;
use std::os::raw::c_void;
use std::thread;

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
pub struct Context {
    pub(crate) egl_context: EGLContext,
    pub(crate) id: ContextID,
    pub(crate) pbuffer: EGLSurface,
    framebuffer: Framebuffer<Surface, ExternalEGLSurfaces>,
    context_is_owned: bool,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if self.egl_context != egl::NO_CONTEXT && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

impl Device {
    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    #[inline]
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        unsafe {
            ContextDescriptor::new(
                self.egl_display,
                attributes,
                &[
                    egl::COLOR_BUFFER_TYPE as EGLint,
                    egl::RGB_BUFFER as EGLint,
                    egl::SURFACE_TYPE as EGLint,
                    egl::PBUFFER_BIT as EGLint,
                    egl::RENDERABLE_TYPE as EGLint,
                    egl::OPENGL_ES2_BIT as EGLint,
                ],
            )
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(
        &mut self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let egl_display = self.egl_display;

        unsafe {
            // Create the EGL context.
            let gl_api = self.gl_api();
            let egl_context = context::create_context(
                egl_display,
                descriptor,
                share_with.map_or(egl::NO_CONTEXT, |ctx| ctx.egl_context),
                gl_api,
            )?;

            // Create a dummy pbuffer.
            let pbuffer = context::create_dummy_pbuffer(egl_display, egl_context);

            // Wrap up the EGL context.
            let context = Context {
                egl_context,
                id: *next_context_id,
                pbuffer,
                framebuffer: Framebuffer::None,
                context_is_owned: true,
            };
            next_context_id.0 += 1;
            Ok(context)
        }
    }

    /// Wraps a native `EGLContext` in a context object.
    ///
    /// The underlying `EGLContext` is not retained, as there is no way to do this in the EGL API.
    /// Therefore, it is the caller's responsibility to keep it alive as long as this `Context`
    /// remains alive.
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext,
    ) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Create a dummy pbuffer.
        let pbuffer = context::create_dummy_pbuffer(self.egl_display, native_context.egl_context);

        // Create the context.
        let context = Context {
            egl_context: native_context.egl_context,
            id: *next_context_id,
            pbuffer,
            framebuffer: Framebuffer::External(ExternalEGLSurfaces {
                draw: native_context.egl_draw_surface,
                read: native_context.egl_read_surface,
            }),
            context_is_owned: false,
        };
        next_context_id.0 += 1;

        Ok(context)
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.egl_context == egl::NO_CONTEXT {
            return Ok(());
        }

        unsafe {
            if let Framebuffer::Surface(mut target) =
                mem::replace(&mut context.framebuffer, Framebuffer::None)
            {
                self.destroy_surface(context, &mut target)?;
            }

            EGL_FUNCTIONS.with(|egl| {
                let result = egl.DestroySurface(self.egl_display, context.pbuffer);
                assert_ne!(result, egl::FALSE);
                context.pbuffer = egl::NO_SURFACE;

                egl.MakeCurrent(
                    self.egl_display,
                    egl::NO_SURFACE,
                    egl::NO_SURFACE,
                    egl::NO_CONTEXT,
                );

                if context.context_is_owned {
                    let result = egl.DestroyContext(self.egl_display, context.egl_context);
                    assert_ne!(result, egl::FALSE);
                }

                context.egl_context = egl::NO_CONTEXT;
            });
        }

        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        GL_FUNCTIONS.with(|gl| unsafe {
            ContextDescriptor::from_egl_context(gl, self.egl_display, context.egl_context)
        })
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let egl_display = self.egl_display;
            let egl_context = context.egl_context;

            let (egl_draw_surface, egl_read_surface) = match context.framebuffer {
                Framebuffer::Surface(Surface {
                    objects: SurfaceObjects::Window { egl_surface },
                    ..
                }) => (egl_surface, egl_surface),
                Framebuffer::External(ExternalEGLSurfaces { draw, read }) => (draw, read),
                Framebuffer::Surface(Surface {
                    objects: SurfaceObjects::HardwareBuffer { .. },
                    ..
                })
                | Framebuffer::None => (context.pbuffer, context.pbuffer),
            };

            EGL_FUNCTIONS.with(|egl| {
                let result =
                    egl.MakeCurrent(egl_display, egl_draw_surface, egl_read_surface, egl_context);
                if result == egl::FALSE {
                    let err = egl.GetError().to_windowing_api_error();
                    return Err(Error::MakeCurrentFailed(err));
                }
                Ok(())
            })
        }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe { context::make_no_context_current(self.egl_display) }
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
    pub fn bind_surface_to_context(
        &self,
        context: &mut Context,
        new_surface: Surface,
    ) -> Result<(), (Error, Surface)> {
        if context.id != new_surface.context_id {
            return Err((Error::IncompatibleSurface, new_surface));
        }

        match context.framebuffer {
            Framebuffer::External { .. } => return Err((Error::ExternalRenderTarget, new_surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, new_surface)),
            Framebuffer::None => {}
        }

        context.framebuffer = Framebuffer::Surface(new_surface);
        Ok(())
    }

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context,
    ) -> Result<Option<Surface>, Error> {
        match context.framebuffer {
            Framebuffer::External { .. } => return Err(Error::ExternalRenderTarget),
            Framebuffer::None => return Ok(None),
            Framebuffer::Surface(_) => {}
        }

        // Make sure all changes are synchronized.
        //
        // FIXME(pcwalton): Is this necessary?
        let _guard = self.temporarily_make_context_current(context)?;
        GL_FUNCTIONS.with(|gl| unsafe {
            gl.Flush();
        });

        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => return Ok(Some(surface)),
            Framebuffer::External { .. } | Framebuffer::None => unreachable!(),
        }
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        unsafe { context_descriptor.attributes(self.egl_display) }
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

    pub(crate) fn context_to_egl_config(&self, context: &Context) -> EGLConfig {
        unsafe {
            context::egl_config_from_id(
                self.egl_display,
                context::get_context_attr(
                    self.egl_display,
                    context.egl_context,
                    egl::CONFIG_ID as EGLint,
                ),
            )
        }
    }

    pub(crate) fn temporarily_make_context_current(
        &self,
        context: &Context,
    ) -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External { .. } => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }

    /// Given a context, returns its underlying EGL context and attached surfaces.
    pub fn native_context(&self, context: &Context) -> NativeContext {
        let (egl_draw_surface, egl_read_surface) = match context.framebuffer {
            Framebuffer::Surface(Surface {
                objects: SurfaceObjects::Window { egl_surface },
                ..
            }) => (egl_surface, egl_surface),
            Framebuffer::External(ExternalEGLSurfaces { draw, read }) => (draw, read),
            Framebuffer::Surface(Surface {
                objects: SurfaceObjects::HardwareBuffer { .. },
                ..
            })
            | Framebuffer::None => (context.pbuffer, context.pbuffer),
        };

        NativeContext {
            egl_context: context.egl_context,
            egl_draw_surface,
            egl_read_surface,
        }
    }
}
