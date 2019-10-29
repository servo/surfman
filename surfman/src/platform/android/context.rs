// surfman/surfman/src/platform/android/context.rs
//
//! Wrapper for EGL contexts on Android.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::egl::types::{EGLConfig, EGLContext, EGLDisplay, EGLSurface, EGLint};
use crate::egl;
use crate::gl::Gl;
use crate::gl::types::GLuint;
use crate::platform::generic::egl::context::{self, CurrentContextGuard, NativeContext};
use crate::platform::generic::egl::context::{OwnedEGLContext, UnsafeEGLContextRef};
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion, SurfaceAccess};
use crate::{SurfaceID, SurfaceType};
use super::device::{Device, UnsafeEGLDisplayRef};
use super::surface::{NativeWidget, Surface, SurfaceObjects};

use euclid::default::Size2D;
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::thread;

thread_local! {
    pub static GL_FUNCTIONS: Gl = Gl::load_with(context::get_proc_address);
}

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    pub(crate) pbuffer: EGLSurface,
    framebuffer: Framebuffer,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.native_context.is_destroyed() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

pub(crate) enum Framebuffer {
    None,
    External {
        egl_draw_surface: EGLSurface,
        egl_read_surface: EGLSurface,
    },
    Surface(Surface),
}

impl Device {
    #[inline]
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        unsafe {
            ContextDescriptor::new(self.native_display.egl_display(), config_attributes, &[
                egl::COLOR_BUFFER_TYPE as EGLint,   egl::RGB_BUFFER as EGLint,
                egl::SURFACE_TYPE as EGLint,        egl::PBUFFER_BIT as EGLint,
                egl::RENDERABLE_TYPE as EGLint,     egl::OPENGL_ES2_BIT as EGLint,
            ])
        }
    }

    /// Opens the device and context corresponding to the current EGL context.
    ///
    /// The native context is not retained, as there is no way to do this in the EGL API. It is
    /// the caller's responsibility to keep it alive for the duration of this context. Be careful
    /// when using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library; for example, by Glutin. It's legal to use this method to wrap a context rendering
    /// to any target: either a window or a pbuffer. The target is opaque to `surfman`; the
    /// library will not modify or try to detect the render target. This means that any of the
    /// methods that query or replace the surface—e.g. `replace_context_surface`—will fail if
    /// called with a context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        EGL_FUNCTIONS.with(|egl| {
            let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

            // Grab the current EGL display and EGL context.
            let egl_display = egl.GetCurrentDisplay();
            if egl_display == egl::NO_DISPLAY {
                return Err(Error::NoCurrentContext);
            }
            let egl_context = egl.GetCurrentContext();
            if egl_context == egl::NO_CONTEXT {
                return Err(Error::NoCurrentContext);
            }
            let native_context = Box::new(UnsafeEGLContextRef { egl_context });

            // Get the current surface.
            let egl_draw_surface = egl.GetCurrentSurface(egl::DRAW as EGLint);
            let egl_read_surface = egl.GetCurrentSurface(egl::READ as EGLint);

            // Create the device wrapper.
            let device = Device { native_display: Box::new(UnsafeEGLDisplayRef { egl_display }) };

            // Create a dummy pbuffer.
            let pbuffer = context::create_dummy_pbuffer(egl_display, native_context.egl_context());

            // Create the context.
            let context = Context {
                native_context,
                id: *next_context_id,
                pbuffer,
                framebuffer: Framebuffer::External { egl_draw_surface, egl_read_surface },
            };
            next_context_id.0 += 1;

            Ok((device, context))
        })
    }

    pub fn create_context(&mut self,
                          descriptor: &ContextDescriptor,
                          surface_access: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let egl_config = self.context_descriptor_to_egl_config(descriptor);
        let egl_display = self.native_display.egl_display();

        unsafe {
            // Create the EGL context.
            let egl_context = context::create_context(egl_display, descriptor)?;

            // Create a dummy pbuffer.
            let pbuffer = create_pbuffer(egl_display, egl_config);

            // Wrap up the EGL context.
            let mut context = Context {
                native_context: Box::new(OwnedEGLContext { egl_context }),
                id: *next_context_id,
                pbuffer,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;

            // Build the initial framebuffer.
            let target = self.create_surface(&context, surface_access, surface_type)?;
            context.framebuffer = Framebuffer::Surface(target);
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        unsafe {
            if let Framebuffer::Surface(target) = mem::replace(&mut context.framebuffer,
                                                               Framebuffer::None) {
                self.destroy_surface(context, target)?;

            }

            EGL_FUNCTIONS.with(|egl| {
                let result = egl.DestroySurface(self.native_display.egl_display(),
                                                context.pbuffer);
                assert_ne!(result, egl::FALSE);
                context.pbuffer = egl::NO_SURFACE;

                context.native_context.destroy(self);
            });
        }

        Ok(())
    }

    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            ContextDescriptor::from_egl_context(self.native_display.egl_display(),
                                                context.native_context.egl_context())
        }
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let egl_display = self.native_display.egl_display();
            let egl_context = context.native_context.egl_context();

            let (egl_draw_surface, egl_read_surface) = match context.framebuffer {
                Framebuffer::Surface(Surface {
                    objects: SurfaceObjects::Window { egl_surface },
                    ..
                }) => (egl_surface, egl_surface),
                Framebuffer::External { egl_draw_surface, egl_read_surface } => {
                    (egl_draw_surface, egl_read_surface)
                }
                Framebuffer::Surface(Surface {
                    objects: SurfaceObjects::HardwareBuffer { .. },
                    ..
                }) | Framebuffer::None => (context.pbuffer, context.pbuffer),
            };

            EGL_FUNCTIONS.with(|egl| {
                let result = egl.MakeCurrent(egl_display,
                                             egl_draw_surface,
                                             egl_read_surface,
                                             egl_context);
                if result == egl::FALSE {
                    let err = egl.GetError().to_windowing_api_error();
                    return Err(Error::MakeCurrentFailed(err));
                }
            });

            Ok(())
        }
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            context::make_no_context_current(self.native_display.egl_display()
        }
    }

    #[inline]
    pub fn present_context_surface(&self, context: &mut Context) -> Result<(), Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External { egl_draw_surface, .. } => {
                EGL_FUNCTIONS.with(|egl| {
                    unsafe {
                        egl.SwapBuffers(self.native_display.egl_display(), egl_draw_surface);
                    }
                });
                Ok(())
            }
            Framebuffer::Surface(ref mut surface) => self.present_surface_without_context(surface),
        }
    }

    fn context_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External { .. } => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref target) => Ok(target),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if let Framebuffer::External { .. }= context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        // Make sure all changes are synchronized.
        //
        // FIXME(pcwalton): Is this necessary?
        self.make_context_current(context)?;
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.Flush();
            }
        });

        let target_slot = match context.framebuffer {
            Framebuffer::None | Framebuffer::External { .. } => unreachable!(),
            Framebuffer::Surface(ref mut target) => target,
        };
        Ok(mem::replace(target_slot, new_surface))
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::Surface(Surface {
                objects: SurfaceObjects::HardwareBuffer { framebuffer_object, .. },
                ..
            }) => Ok(framebuffer_object),
            Framebuffer::Surface(Surface { objects: SurfaceObjects::Window { .. }, .. }) |
            Framebuffer::External { .. } => Ok(0),
        }
    }

    #[inline]
    pub fn context_surface_size(&self, context: &Context) -> Result<Size2D<i32>, Error> {
        self.context_surface(context).map(|surface| surface.size())
    }

    #[inline]
    pub fn context_surface_id(&self, context: &Context) -> Result<SurfaceID, Error> {
        self.context_surface(context).map(|surface| surface.id())
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        unsafe {
            context_descriptor.attributes(self.native_display.egl_display())
        }
    }

    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        context::get_proc_address(symbol_name)
    }

    pub(crate) fn context_descriptor_to_egl_config(&self, context_descriptor: &ContextDescriptor)
                                                   -> EGLConfig {
        unsafe {
            context::egl_config_from_id(self.native_display.egl_display(),
                                        context_descriptor.egl_config_id)
        }
    }

    pub(crate) fn temporarily_make_context_current(&self, context: &Context)
                                                   -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }
}
