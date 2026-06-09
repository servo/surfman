//! A thread-local handle to the device.

use super::connection::Connection;
use crate::base::egl::context::{self, CurrentContextGuard};
use crate::base::egl::device::EGL_FUNCTIONS;
use crate::base::egl::error::ToWindowingApiError;
use crate::base::egl::surface::ExternalEGLSurfaces;
use crate::context::{ContextID, CREATE_CONTEXT_MUTEX};
use crate::egl::types::{EGLConfig, EGLDisplay, EGLint};
use crate::hardware_buffer::surface::SurfaceObjects;
use crate::surface::Framebuffer;
use crate::{egl, ContextDescriptor, NativeContext, Surface};
use crate::{Context, ContextAttributes, Error, GLApi, Gl, SurfaceInfo};
use euclid::default::Size2D;
use glow::HasContext;
use std::mem;
use std::os::raw::c_void;

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub struct Adapter;

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) egl_display: EGLDisplay,
    pub(crate) display_is_owned: bool,
}

/// Wrapper for an `EGLDisplay`.
#[derive(Clone, Copy)]
pub struct NativeDevice(pub EGLDisplay);

impl Drop for Device {
    fn drop(&mut self) {
        EGL_FUNCTIONS.with(|egl| unsafe {
            if !self.display_is_owned {
                return;
            }
            let result = egl.Terminate(self.egl_display);
            assert_ne!(result, egl::FALSE);
            self.egl_display = egl::NO_DISPLAY;
        })
    }
}

impl NativeDevice {
    /// Returns the current EGL display.
    ///
    /// If there is no current EGL display, `egl::NO_DISPLAY` is returned.
    pub fn current() -> NativeDevice {
        EGL_FUNCTIONS.with(|egl| unsafe { NativeDevice(egl.GetCurrentDisplay()) })
    }
}

impl Device {
    #[inline]
    pub(crate) fn new() -> Result<Device, Error> {
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                let egl_display = egl.GetDisplay(egl::DEFAULT_DISPLAY);
                assert_ne!(egl_display, egl::NO_DISPLAY);

                // I don't think this should ever fail.
                let (mut major_version, mut minor_version) = (0, 0);
                let result = egl.Initialize(egl_display, &mut major_version, &mut minor_version);
                assert_ne!(result, egl::FALSE);

                Ok(Device {
                    egl_display,
                    display_is_owned: true,
                })
            }
        })
    }

    /// Returns the EGL display corresponding to this device.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice(self.egl_display)
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }

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
        &self,
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
            let pbuffer = context::create_dummy_pbuffer(egl_display, egl_context).unwrap();

            EGL_FUNCTIONS.with(|egl| {
                if egl.MakeCurrent(egl_display, pbuffer, pbuffer, egl_context) == egl::FALSE {
                    let err = egl.GetError().to_windowing_api_error();
                    return Err(Error::MakeCurrentFailed(err));
                }
                Ok(())
            })?;

            // Wrap up the EGL context.
            let context = Context {
                egl_context,
                id: *next_context_id,
                pbuffer,
                framebuffer: Framebuffer::None,
                context_is_owned: true,
                gl: Gl::from_loader_function(context::get_proc_address),
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
        let pbuffer =
            context::create_dummy_pbuffer(self.egl_display, native_context.egl_context).unwrap();

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
            gl: Gl::from_loader_function(context::get_proc_address),
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
        unsafe {
            ContextDescriptor::from_egl_context(&context.gl, self.egl_display, context.egl_context)
        }
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
                }) => (context.pbuffer, context.pbuffer),
                Framebuffer::None => (context.pbuffer, context.pbuffer),
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
        unsafe {
            context.gl.flush();
        };

        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => return Ok(Some(surface)),
            Framebuffer::External { .. } | Framebuffer::None => unreachable!(),
        }
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        match &context.framebuffer {
            Framebuffer::Surface(surface) => self.present_surface_inner(context, surface),
            _ => Ok(()),
        }
    }

    /// If the currently bound surface is a widget surface, resize it,
    pub fn resize_bound_surface(
        &self,
        context: &mut Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        if let Framebuffer::Surface(surface) = &mut context.framebuffer {
            surface.resize(size);
        }
        Ok(())
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
            }) => (context.pbuffer, context.pbuffer),
            Framebuffer::None => (context.pbuffer, context.pbuffer),
        };

        NativeContext {
            egl_context: context.egl_context,
            egl_draw_surface,
            egl_read_surface,
        }
    }
}
