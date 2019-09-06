//! Wrapper for Core OpenGL contexts.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLFlavor, GLInfo, GLVersion};
use super::adapter::Adapter;
use super::device::Device;
use super::error::ToWindowingApiError;
use super::surface::{Framebuffer, Renderbuffers, Surface, SurfaceTexture};
use cgl::{CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDescribePixelFormat};
use cgl::{CGLDestroyContext, CGLError, CGLGetCurrentContext, CGLGetPixelFormat};
use cgl::{CGLPixelFormatAttribute, CGLPixelFormatObj, CGLSetCurrentContext, kCGLPFAAlphaSize};
use cgl::{kCGLPFADepthSize, kCGLPFAStencilSize, kCGLPFAOpenGLProfile};
use core_foundation::base::TCFType;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;
use gl;
use gl::types::GLuint;
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
    pub(crate) native_context: Box<dyn NativeContext>,
    gl_info: GLInfo,
    framebuffer: Framebuffer,
}

pub(crate) trait NativeContext {
    fn cgl_context(&self) -> CGLContextObj;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
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
    /// Opens the device and context corresponding to the current CGL context.
    ///
    /// The native context is not retained, as there is no way to do this in the CGL API. It is the
    /// caller's responsibility to keep it alive for the duration of this context. Be careful when
    /// using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library; for example, by Glutin. It's legal to use this method to wrap a context rendering
    /// to any target: either a window or a pbuffer. The target is opaque to `surfman`; the library
    /// will not modify or try to detect the render target. This means that any of the methods that
    /// query or replace the surface—e.g. `replace_context_color_surface`—will fail if called with
    /// a context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        let mut previous_context_created = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Grab the current context.
        let native_context = Box::new(UnsafeCGLContextRef::current());
        println!("Device::from_current_context() = {:x}", native_context.cgl_context() as usize);

        // Detect context attributes.
        let pixel_format = CGLGetPixelFormat(native_context.cgl_context());
        debug_assert_ne!(pixel_format, ptr::null_mut());

        let alpha_size = get_pixel_format_attribute(pixel_format, kCGLPFAAlphaSize);
        let depth_size = get_pixel_format_attribute(pixel_format, kCGLPFADepthSize);
        let stencil_size = get_pixel_format_attribute(pixel_format, kCGLPFAStencilSize);
        let gl_profile = get_pixel_format_attribute(pixel_format, kCGLPFAOpenGLProfile);

        let mut attribute_flags = ContextAttributeFlags::empty();
        attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
        attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
        attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

        let version = if gl_profile == kCGLOGLPVersion_Legacy {
            GLVersion::new(2, 0)
        } else {
            GLVersion::new(4, 2)
        };

        let attributes = ContextAttributes {
            flags: attribute_flags,
            flavor: GLFlavor { api: GLApi::GL, version },
        };

        let mut context = Context {
            native_context,
            gl_info: GLInfo::new(&attributes),
            framebuffer: Framebuffer::External,
        };

        let device = Device::new(&Adapter)?;

        if !*previous_context_created {
            gl::load_with(|symbol| {
                device.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
            });
            *previous_context_created = true;
        }

        context.gl_info.populate();
        return Ok((device, context));

        unsafe fn get_pixel_format_attribute(pixel_format: CGLPixelFormatObj,
                                             attribute: CGLPixelFormatAttribute)
                                             -> i32 {
            let mut value = 0;
            let err = CGLDescribePixelFormat(pixel_format, 0, attribute, &mut value);
            debug_assert_eq!(err, kCGLNoError);
            value
        }
    }

    pub fn create_context(&self, attributes: &ContextAttributes) -> Result<Context, Error> {
        if attributes.flavor.api == GLApi::GLES {
            return Err(Error::UnsupportedGLType);
        }

        // Take a lock so that we're only creating one context at a time. This serves two purposes:
        //
        // 1. CGLChoosePixelFormat fails, returning `kCGLBadConnection`, if multiple threads try to
        //    open a display connection simultaneously.
        // 2. The first thread to create a context needs to load the GL function pointers.
        let mut previous_context_created = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let profile = if attributes.flavor.version.major >= 3 {
            kCGLOGLPVersion_3_2_Core
        } else {
            kCGLOGLPVersion_Legacy
        };

        let pixel_format_attributes = [
            kCGLPFAOpenGLProfile, profile,
            0, 0,
        ];

        unsafe {
            let (mut pixel_format, mut pixel_format_count) = (ptr::null_mut(), 0);
            let mut err = CGLChoosePixelFormat(pixel_format_attributes.as_ptr(),
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
            let native_context = Box::new(OwnedCGLContext { cgl_context });

            let err = CGLSetCurrentContext(native_context.cgl_context());
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }

            println!("Device::create_context() = {:x}", native_context.cgl_context() as usize);

            let mut context = Context {
                native_context,
                framebuffer: Framebuffer::None,
                gl_info: GLInfo::new(attributes),
            };

            if !*previous_context_created {
                gl::load_with(|symbol| {
                    self.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
                });
                *previous_context_created = true;
            }

            context.gl_info.populate();
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        let mut result = Ok(());
        if context.native_context.is_destroyed() {
            return result;
        }

        if let Framebuffer::Object {
            framebuffer_object,
            mut renderbuffers,
            color_surface_texture,
        } = mem::replace(&mut context.framebuffer, Framebuffer::None) {
            renderbuffers.destroy();

            if framebuffer_object != 0 {
                unsafe {
                    gl::DeleteFramebuffers(1, &framebuffer_object);
                }
            }

            match self.destroy_surface_texture(context, color_surface_texture) {
                Err(err) => result = Err(err),
                Ok(surface) => {
                    if let Err(err) = self.destroy_surface(context, surface) {
                        result = Err(err);
                    }
                }
            }
        }

        unsafe {
            context.native_context.destroy();
        }

        result
    }

    #[inline]
    pub fn context_gl_info<'c>(&self, context: &'c Context) -> &'c GLInfo {
        &context.gl_info
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(context.native_context.cgl_context());
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
        }
    }

    pub fn make_context_not_current(&self, _: &Context) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(ptr::null_mut());
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
        }
    }

    pub fn get_proc_address(&self, _: &Context, symbol_name: &str)
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
            Framebuffer::None | Framebuffer::External => None,
            Framebuffer::Object { ref color_surface_texture, .. } => {
                Some(&color_surface_texture.surface)
            }
        }
    }

    pub fn replace_context_color_surface(&self, context: &mut Context, new_color_surface: Surface)
                                         -> Result<Option<Surface>, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        self.make_context_current(context)?;

        // Make sure all changes are synchronized. Apple requires this.
        unsafe {
            gl::Flush();
        }

        // Fast path: we have a FBO set up already and the sizes are the same. In this case, we can
        // just switch the backing texture.
        let can_modify_existing_framebuffer = match context.framebuffer {
            Framebuffer::Object { ref color_surface_texture, .. } => {
                // FIXME(pcwalton): Should we check parts of the descriptor other than size as
                // well?
                color_surface_texture.surface().descriptor().size ==
                    new_color_surface.descriptor().size
            }
            Framebuffer::None | Framebuffer::External => false,
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

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => Err(Error::NoSurfaceAttached),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Object { framebuffer_object, .. } => Ok(framebuffer_object),
        }
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

            // Set the viewport so that the application doesn't have to do so explicitly.
            gl::Viewport(0, 0, descriptor.size.width, descriptor.size.height);

            context.framebuffer = Framebuffer::Object {
                framebuffer_object,
                color_surface_texture,
                renderbuffers,
            };
        }

        Ok(())
    }

    fn destroy_framebuffer(&self, context: &mut Context) -> (Option<Surface>, Result<(), Error>) {
        let (framebuffer_object,
             color_surface_texture,
             mut renderbuffers) = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::External => unreachable!(),
            Framebuffer::None => return (None, Ok(())),
            Framebuffer::Object { framebuffer_object, color_surface_texture, renderbuffers } => {
                (framebuffer_object, color_surface_texture, renderbuffers)
            }
        };

        let old_surface = match self.destroy_surface_texture(context, color_surface_texture) {
            Ok(old_surface) => old_surface,
            Err(err) => return (None, Err(err)),
        };

        renderbuffers.destroy();

        unsafe {
            gl::DeleteFramebuffers(1, &framebuffer_object);
        }

        (Some(old_surface), Ok(()))
    }

    fn replace_color_surface_in_existing_framebuffer(&self,
                                                     context: &mut Context,
                                                     new_color_surface: Surface)
                                                     -> Result<Surface, Error> {
        println!("replace_color_surface_in_existing_framebuffer()");
        let new_color_surface_texture = self.create_surface_texture(context, new_color_surface)?;

        let (framebuffer_object, framebuffer_color_surface_texture) = match context.framebuffer {
            Framebuffer::Object { framebuffer_object, ref mut color_surface_texture, .. } => {
                (framebuffer_object, color_surface_texture)
            }
            _ => unreachable!(),
        };

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     SurfaceTexture::gl_texture_target(),
                                     new_color_surface_texture.gl_texture(),
                                     0);
        }

        let old_color_surface_texture = mem::replace(framebuffer_color_surface_texture,
                                                     new_color_surface_texture);
        self.destroy_surface_texture(context, old_color_surface_texture)
    }
}

struct OwnedCGLContext {
    cgl_context: CGLContextObj,
}

impl NativeContext for OwnedCGLContext {
    #[inline]
    fn cgl_context(&self) -> CGLContextObj {
        debug_assert!(!self.is_destroyed());
        self.cgl_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.cgl_context.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        CGLSetCurrentContext(ptr::null_mut());
        CGLDestroyContext(self.cgl_context);
        self.cgl_context = ptr::null_mut();
    }
}

struct UnsafeCGLContextRef {
    cgl_context: CGLContextObj,
}

impl UnsafeCGLContextRef {
    #[inline]
    unsafe fn current() -> UnsafeCGLContextRef {
        let cgl_context = CGLGetCurrentContext();
        assert!(!cgl_context.is_null());
        UnsafeCGLContextRef { cgl_context }
    }
}

impl NativeContext for UnsafeCGLContextRef {
    #[inline]
    fn cgl_context(&self) -> CGLContextObj {
        self.cgl_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.cgl_context.is_null()
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.cgl_context = ptr::null_mut();
    }
}
