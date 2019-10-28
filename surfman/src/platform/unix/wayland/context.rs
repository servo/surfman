// surfman/surfman/src/platform/unix/wayland/context.rs
//
//! A wrapper around Wayland `EGLContext`s.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::egl::types::{EGLConfig, EGLint};
use crate::egl;
use crate::gl::Gl;
use crate::gl::types::GLuint;
use crate::platform::generic::egl::context::{self, CurrentContextGuard, NativeContext};
use crate::platform::generic::egl::context::{OwnedEGLContext, UnsafeEGLContextRef};
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::platform::generic::egl::ffi::EGL_FUNCTIONS;
use crate::surface::Framebuffer;
use crate::{ContextAttributes, Error, SurfaceAccess, SurfaceID, SurfaceType};
use super::device::Device;
use super::surface::{NativeWidget, Surface, WaylandObjects};

use euclid::default::Size2D;
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
    framebuffer: Framebuffer<Surface>,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.native_context.is_destroyed() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

impl Device {
    #[inline]
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        unsafe {
            ContextDescriptor::new(self.native_connection.egl_display(), attributes, &[
                egl::SURFACE_TYPE as EGLint,    egl::WINDOW_BIT as EGLint,
                egl::RENDERABLE_TYPE as EGLint, egl::OPENGL_BIT as EGLint,
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
    ///
    /// TODO(pcwalton)
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        /*
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Grab the current EGL display and EGL context.
        let egl_display = EGL_FUNCTIONS.GetCurrentDisplay();
        if egl_display == egl::NO_DISPLAY {
            return Err(Error::NoCurrentContext);
        }
        let egl_context = EGL_FUNCTIONS.GetCurrentContext();
        if egl_context == egl::NO_CONTEXT {
            return Err(Error::NoCurrentContext);
        }
        let native_context = Box::new(UnsafeEGLContextRef { egl_context });

        // Create the context.
        let mut context = Context {
            native_context,
            id: *next_context_id,
            framebuffer: Framebuffer::External,
        };
        next_context_id.0 += 1;

        Ok((device, context))
        */
        unimplemented!()
    }

    pub fn create_context(&mut self,
                          descriptor: &ContextDescriptor,
                          surface_access: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        unsafe {
            // Create the context.
            let egl_display = self.native_connection.egl_display();
            let egl_context = context::create_context(egl_display, descriptor)?;

            // Wrap the context.
            let mut context = Context {
                native_context: Box::new(OwnedEGLContext { egl_context }),
                id: *next_context_id,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;

            // Build the initial surface.
            let initial_surface = match self.create_surface(&context,
                                                            surface_access,
                                                            surface_type) {
                Ok(surface) => surface,
                Err(err) => {
                    self.destroy_context(&mut context)?;
                    return Err(err);
                }
            };

            self.attach_surface(&mut context, initial_surface);

            // Return the context.
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Some(surface) = self.release_surface(context) {
            self.destroy_surface(context, surface)?;
        }

        unsafe {
            context.native_context.destroy(self.native_connection.egl_display());
        }

        Ok(())
    }

    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            ContextDescriptor::from_egl_context(self.native_connection.egl_display(),
                                                context.native_context.egl_context())
        }
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let egl_surface = match context.framebuffer {
                Framebuffer::Surface(Surface {
                    wayland_objects: WaylandObjects::Window { egl_surface, .. },
                    ..
                }) => egl_surface,
                Framebuffer::Surface(Surface {
                    wayland_objects: WaylandObjects::TextureImage { .. },
                    ..
                }) | Framebuffer::None => egl::NO_SURFACE,
                Framebuffer::External => {
                    return Err(Error::ExternalRenderTarget)
                }
            };
            let result = EGL_FUNCTIONS.MakeCurrent(self.native_connection.egl_display(),
                                                   egl_surface,
                                                   egl_surface,
                                                   context.native_context.egl_context());
            if result == egl::FALSE {
                let err = EGL_FUNCTIONS.GetError().to_windowing_api_error();
                return Err(Error::MakeCurrentFailed(err));
            }

            Ok(())
        }
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            context::make_no_context_current(self.native_connection.egl_display())
        }
    }

    pub(crate) fn temporarily_make_context_current(&self, context: &Context)
                                                   -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    pub(crate) fn context_is_current(&self, context: &Context) -> bool {
        unsafe {
            EGL_FUNCTIONS.GetCurrentContext() == context.native_context.egl_context()
        }
    }

    fn context_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface),
        }
    }

    fn context_surface_mut<'c>(&self, context: &'c mut Context)
                               -> Result<&'c mut Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref mut surface) => Ok(surface),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        let is_current = self.context_is_current(context);
        let old_surface = self.release_surface(context).expect("Where's our surface?");
        self.attach_surface(context, new_surface);

        if is_current {
            // We need to make ourselves current again, because the surface changed.
            self.make_context_current(context)?;
        }

        Ok(old_surface)
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::Surface(Surface {
                wayland_objects: WaylandObjects::TextureImage { framebuffer_object, .. },
                ..
            }) => Ok(framebuffer_object),
            Framebuffer::Surface(Surface { wayland_objects: WaylandObjects::Window { .. }, .. }) |
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

    #[inline]
    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        unsafe {
            context_descriptor.attributes(self.native_connection.egl_display())
        }
    }

    #[inline]
    pub fn present_context_surface(&self, context: &mut Context) -> Result<(), Error> {
        self.context_surface_mut(context).and_then(|surface| {
            self.present_surface_without_context(surface)
        })
    }

    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        context::get_proc_address(symbol_name)
    }

    #[inline]
    pub(crate) fn context_descriptor_to_egl_config(&self, context_descriptor: &ContextDescriptor)
                                                   -> EGLConfig {
        unsafe {
            context::egl_config_from_id(self.native_connection.egl_display(),
                                        context_descriptor.egl_config_id)
        }
    }

    fn attach_surface(&self, context: &mut Context, surface: Surface) {
        match context.framebuffer {
            Framebuffer::None => {}
            _ => panic!("Tried to attach a surface, but there was already a surface present!"),
        }

        context.framebuffer = Framebuffer::Surface(surface);
    }

    fn release_surface(&self, context: &mut Context) -> Option<Surface> {
        let surface = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => surface,
            Framebuffer::None | Framebuffer::External => return None,
        };

        Some(surface)
    }
}
