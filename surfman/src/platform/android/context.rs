// surfman/surfman/src/platform/android/context.rs
//
//! Wrapper for EGL contexts on Android.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::egl::types::{EGLConfig, EGLSurface, EGLint};
use crate::egl;
use crate::gl::Gl;
use crate::platform::generic::egl::context::{self, CurrentContextGuard, NativeContext};
use crate::platform::generic::egl::context::{OwnedEGLContext, UnsafeEGLContextRef};
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::{ContextAttributes, Error, SurfaceInfo};
use super::device::{Device, UnsafeEGLDisplayRef};
use super::surface::{Surface, SurfaceObjects};

use std::mem;
use std::os::raw::c_void;
use std::thread;

pub use crate::platform::generic::egl::context::ContextDescriptor;

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
            ContextDescriptor::new(self.native_display.egl_display(), attributes, &[
                egl::COLOR_BUFFER_TYPE as EGLint,   egl::RGB_BUFFER as EGLint,
                egl::SURFACE_TYPE as EGLint,        egl::PBUFFER_BIT as EGLint,
                egl::RENDERABLE_TYPE as EGLint,     egl::OPENGL_ES2_BIT as EGLint,
            ])
        }
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let egl_config = self.context_descriptor_to_egl_config(descriptor);
        let egl_display = self.native_display.egl_display();

        unsafe {
            // Create the EGL context.
            let egl_context = context::create_context(egl_display, descriptor)?;

            // Create a dummy pbuffer.
            let pbuffer = context::create_dummy_pbuffer(egl_display, egl_config);

            // Wrap up the EGL context.
            let context = Context {
                native_context: Box::new(OwnedEGLContext { egl_context }),
                id: *next_context_id,
                pbuffer,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;
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

                context.native_context.destroy(self.native_display.egl_display());
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
                Ok(())
            })
        }
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            context::make_no_context_current(self.native_display.egl_display())
        }
    }

    pub fn bind_surface_to_context(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<(), Error> {
        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        match context.framebuffer {
            Framebuffer::External { .. } => return Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(_) => return Err(Error::SurfaceAlreadyBound),
            Framebuffer::None => {}
        }

        context.framebuffer = Framebuffer::Surface(new_surface);
        Ok(())
    }

    pub fn unbind_surface_from_context(&self, context: &mut Context)
                                       -> Result<Option<Surface>, Error> {
        match context.framebuffer {
            Framebuffer::External { .. } => return Err(Error::ExternalRenderTarget),
            Framebuffer::None => return Ok(None),
            Framebuffer::Surface(_) => {}
        }

        // Make sure all changes are synchronized.
        //
        // FIXME(pcwalton): Is this necessary?
        let _guard = self.temporarily_make_context_current(context)?;
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.Flush();
            }
        });

        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => return Ok(Some(surface)),
            Framebuffer::External { .. } | Framebuffer::None => unreachable!(),
        }
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

    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }

    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External { .. } => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }
}
