//! Wrapper for Core OpenGL contexts.

use crate::platform::with_cgl::error::ToWindowingApiError;
use crate::{Error, GLApi, GLInfo};
use super::device::Device;
use super::surface::{Framebuffer, Renderbuffers, Surface, SurfaceTexture};
use cgl::{CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDestroyContext, CGLError};
use cgl::{CGLPixelFormatAttribute, CGLSetCurrentContext, kCGLPFAOpenGLProfile};
use core_foundation::base::TCFType;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;
use gl;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;

// No CGL error occurred.
#[allow(non_upper_case_globals)]
const kCGLNoError: CGLError = 0;

lazy_static! {
    static ref CREATE_CONTEXT_MUTEX: Mutex<bool> = Mutex::new(false);
}

// CGL OpenGL Profile that chooses a Legacy/Pre-OpenGL 3.0 Implementation.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_Legacy: CGLPixelFormatAttribute = 0x1000;
// CGL OpenGL Profile that chooses a Legacy/Pre-OpenGL 3.0 Implementation.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_3_2_Core: CGLPixelFormatAttribute = 0x3200;

pub struct Context {
    pub(crate) cgl_context: CGLContextObj,
    gl_info: GLInfo,
    framebuffer: Option<Framebuffer>,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.cgl_context.is_null() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

impl Device {
    pub fn create_context(&self, gl_info: &GLInfo) -> Result<Context, Error> {
        if gl_info.flavor.api_type == GLApi::GLES {
            return Err(Error::UnsupportedGLType);
        }

        // Take a lock so that we're only creating one context at a time. This serves two purposes:
        //
        // 1. CGLChoosePixelFormat fails, returning `kCGLBadConnection`, if multiple threads try to
        //    open a display connection simultaneously.
        // 2. The first thread to create a context needs to load the GL function pointers.
        let mut previous_context_created = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let profile = if gl_info.flavor.api_version.major_version() >= 3 {
            kCGLOGLPVersion_3_2_Core
        } else {
            kCGLOGLPVersion_Legacy
        };

        let attributes = [
            kCGLPFAOpenGLProfile, profile,
            0, 0,
        ];

        unsafe {
            let (mut pixel_format, mut pixel_format_count) = (ptr::null_mut(), 0);
            let mut err = CGLChoosePixelFormat(attributes.as_ptr(),
                                               &mut pixel_format,
                                               &mut pixel_format_count);
            if err != kCGLNoError {
                return Err(Error::PixelFormatSelectionFailed(err.to_windowing_api_error()));
            }
            if pixel_format_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            let mut cgl_context = ptr::null_mut();
            err = CGLCreateContext(pixel_format, ptr::null_mut(), &mut cgl_context);
            if err != kCGLNoError {
                return Err(Error::ContextCreationFailed(err.to_windowing_api_error()));
            }

            debug_assert_ne!(cgl_context, ptr::null_mut());

            let mut context = Context { cgl_context, framebuffer: None, gl_info: *gl_info };
            if !*previous_context_created {
                gl::load_with(|symbol| {
                    self.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
                });
                *previous_context_created = true;
            }

            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        let mut result = Ok(());
        if context.cgl_context.is_null() {
            return result;
        }

        if let Some(mut framebuffer) = context.framebuffer.take() {
            framebuffer.renderbuffers.destroy();

            if framebuffer.framebuffer_object != 0 {
                unsafe {
                    gl::DeleteFramebuffers(1, &framebuffer.framebuffer_object);
                    framebuffer.framebuffer_object = 0;
                }
            }

            match self.destroy_surface_texture(context, framebuffer.color_surface_texture) {
                Err(err) => result = Err(err),
                Ok(surface) => {
                    if let Err(err) = self.destroy_surface(context, surface) {
                        result = Err(err);
                    }
                }
            }
        }

        if let Err(err) = self.make_context_not_current(context) {
            result = Err(err);
        }

        unsafe {
            let err = CGLDestroyContext(context.cgl_context);
            context.cgl_context = ptr::null_mut();
            if err != kCGLNoError {
                result = Err(Error::ContextDestructionFailed(err.to_windowing_api_error()));
            }
        }

        result
    }

    #[inline]
    pub fn context_gl_info<'c>(&self, context: &'c Context) -> &'c GLInfo {
        &context.gl_info
    }

    pub fn make_context_current(&self, context: &mut Context) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(context.cgl_context);
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
        }
    }

    pub fn make_context_not_current(&self, _: &mut Context) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(ptr::null_mut());
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
        }
    }

    pub fn get_proc_address(&self, _: &mut Context, symbol_name: &str)
                            -> Result<*const c_void, Error> {
        unsafe {
            let framework_identifier: CFString =
                FromStr::from_str(OPENGL_FRAMEWORK_IDENTIFIER).unwrap();
            let framework =
                CFBundleGetBundleWithIdentifier(framework_identifier.as_concrete_TypeRef());
            if framework.is_null() {
                return Err(Error::NoGLLibraryFound);
            }

            let symbol_name: CFString = FromStr::from_str(symbol_name).unwrap();
            let fun_ptr = CFBundleGetFunctionPointerForName(framework,
                                                            symbol_name.as_concrete_TypeRef());
            if fun_ptr.is_null() {
                return Err(Error::GLFunctionNotFound);
            }
            
            return Ok(fun_ptr as *const c_void);
        }

        static OPENGL_FRAMEWORK_IDENTIFIER: &'static str = "com.apple.opengl";
    }

    #[inline]
    pub fn context_color_surface<'c>(&self, context: &'c Context) -> Option<&'c Surface> {
        match context.framebuffer {
            None => None,
            Some(ref framebuffer) => Some(&framebuffer.color_surface_texture.surface),
        }
    }

    pub fn replace_context_color_surface(&self, context: &mut Context, new_color_surface: Surface)
                                         -> Result<Option<Surface>, Error> {
        self.make_context_current(context)?;

        // Fast path: we have a FBO set up already and the sizes are the same. In this case, we can
        // just switch the backing texture.
        let can_modify_existing_framebuffer = match context.framebuffer {
            Some(ref framebuffer) => {
                // FIXME(pcwalton): Should we check parts of the descriptor other than size as
                // well?
                framebuffer.color_surface_texture.surface().descriptor().size ==
                    new_color_surface.descriptor().size
            }
            None => false,
        };
        if can_modify_existing_framebuffer {
            return self.replace_color_surface_in_existing_framebuffer(context, new_color_surface)
                       .map(Some);
        }

        let (old_surface, result) = self.destroy_framebuffer(context);
        if let Err(err) = result {
            if let Some(old_surface) = old_surface {
                drop(self.destroy_surface(context, old_surface));
            }
            return Err(err);
        }
        if let Err(err) = self.create_framebuffer(context, new_color_surface) {
            if let Some(old_surface) = old_surface {
                drop(self.destroy_surface(context, old_surface));
            }
            return Err(err);
        }

        Ok(old_surface)
    }

    // Assumes that the context is current.
    fn create_framebuffer(&self, context: &mut Context, color_surface: Surface)
                          -> Result<(), Error> {
        let descriptor = *color_surface.descriptor();
        let color_surface_texture = self.create_surface_texture(context, color_surface)?;

        unsafe {
            let mut framebuffer_object = 0;
            gl::GenFramebuffers(1, &mut framebuffer_object);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                    gl::COLOR_ATTACHMENT0,
                                    SurfaceTexture::gl_texture_target(),
                                    color_surface_texture.gl_texture(),
                                    0);

            let renderbuffers = Renderbuffers::new(&descriptor.size, &context.gl_info);
            renderbuffers.bind_to_current_framebuffer();

            debug_assert_eq!(gl::CheckFramebufferStatus(gl::FRAMEBUFFER),
                             gl::FRAMEBUFFER_COMPLETE);

            context.framebuffer = Some(Framebuffer {
                framebuffer_object,
                color_surface_texture,
                renderbuffers,
            });
        }

        Ok(())
    }

    fn destroy_framebuffer(&self, context: &mut Context) -> (Option<Surface>, Result<(), Error>) {
        let mut framebuffer = match context.framebuffer.take() {
            None => return (None, Ok(())),
            Some(framebuffer) => framebuffer,
        };

        let old_surface = match self.destroy_surface_texture(context,
                                                             framebuffer.color_surface_texture) {
            Ok(old_surface) => old_surface,
            Err(err) => return (None, Err(err)),
        };

        framebuffer.renderbuffers.destroy();

        unsafe {
            gl::DeleteFramebuffers(1, &framebuffer.framebuffer_object);
        }

        (Some(old_surface), Ok(()))
    }

    fn replace_color_surface_in_existing_framebuffer(&self,
                                                     context: &mut Context,
                                                     new_color_surface: Surface)
                                                     -> Result<Surface, Error> {
        let new_color_surface_texture = self.create_surface_texture(context, new_color_surface)?;

        let framebuffer = context.framebuffer.as_mut().unwrap();
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer.framebuffer_object);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                    gl::COLOR_ATTACHMENT0,
                                    SurfaceTexture::gl_texture_target(),
                                    new_color_surface_texture.gl_texture(),
                                    0);
        }

        let old_color_surface_texture = mem::replace(&mut framebuffer.color_surface_texture,
                                                     new_color_surface_texture);
        self.destroy_surface_texture(context, old_color_surface_texture)
    }
}
