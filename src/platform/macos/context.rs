//! Wrapper for Core OpenGL contexts.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLFlavor, GLInfo, GLVersion};
use super::adapter::Adapter;
use super::device::Device;
use super::error::ToWindowingApiError;
use super::surface::{Framebuffer, Renderbuffers, Surface, SurfaceTexture};
use cgl::{CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDescribePixelFormat};
use cgl::{CGLDestroyContext, CGLError, CGLGetCurrentContext, CGLGetPixelFormat};
use cgl::{CGLPixelFormatAttribute, CGLPixelFormatObj, CGLReleasePixelFormat, CGLRetainPixelFormat};
use cgl::{CGLSetCurrentContext, kCGLPFAAlphaSize, kCGLPFADepthSize};
use cgl::{kCGLPFAStencilSize, kCGLPFAOpenGLProfile};
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

// Choose a renderer compatible with GL 1.0.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_Legacy: CGLPixelFormatAttribute = 0x1000;
// Choose a renderer capable of GL3.2 or later.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_3_2_Core: CGLPixelFormatAttribute = 0x3200;
// Choose a renderer capable of GL4.1 or later.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_GL4_Core: CGLPixelFormatAttribute = 0x4100;

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    descriptor: ContextDescriptor,
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

pub struct ContextDescriptor {
    cgl_pixel_format: CGLPixelFormatObj,
}

impl Drop for ContextDescriptor {
    // These have been verified to be thread-safe.
    #[inline]
    fn drop(&mut self) {
        unsafe {
            CGLReleasePixelFormat(self.cgl_pixel_format);
        }
    }
}

impl Clone for ContextDescriptor {
    #[inline]
    fn clone(&self) -> ContextDescriptor {
        unsafe {
            ContextDescriptor {
                cgl_pixel_format: CGLRetainPixelFormat(self.cgl_pixel_format),
            }
        }
    }
}

unsafe impl Send for ContextDescriptor {}

impl ContextDescriptor {
}

impl Device {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let profile = if attributes.flavor.version.major >= 4 {
            kCGLOGLPVersion_GL4_Core
        } else if attributes.flavor.version.major == 3 {
            kCGLOGLPVersion_3_2_Core
        } else {
            kCGLOGLPVersion_Legacy
        };

        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        let cgl_pixel_format_attributes = [
            kCGLPFAOpenGLProfile, profile,
            kCGLPFAAlphaSize,     alpha_size,
            kCGLPFADepthSize,     depth_size,
            kCGLPFAStencilSize,   stencil_size,
            0, 0,
        ];

        unsafe {
            let (mut cgl_pixel_format, mut cgl_pixel_format_count) = (ptr::null_mut(), 0);
            let err = CGLChoosePixelFormat(cgl_pixel_format_attributes.as_ptr(),
                                           &mut cgl_pixel_format,
                                           &mut cgl_pixel_format_count);
            if err != kCGLNoError {
                return Err(Error::PixelFormatSelectionFailed(err.to_windowing_api_error()));
            }
            if cgl_pixel_format_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            Ok(ContextDescriptor { cgl_pixel_format })
        }
    }

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

        // Get the context descriptor.
        let cgl_pixel_format = CGLGetPixelFormat(native_context.cgl_context());
        debug_assert_ne!(cgl_pixel_format, ptr::null_mut());
        let descriptor = ContextDescriptor { cgl_pixel_format };

        // Create the context.
        let mut context = Context {
            native_context,
            descriptor,
            gl_info: GLInfo::new(),
            framebuffer: Framebuffer::External,
        };

        let device = Device::new(&Adapter)?;

        if !*previous_context_created {
            gl::load_with(|symbol| {
                device.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
            });
            *previous_context_created = true;
        }

        context.gl_info.populate(&device.context_descriptor_attributes(&context.descriptor));
        Ok((device, context))
    }

    pub fn create_context(&self, color_surface: Surface) -> Result<Context, Error> {
        // Take a lock so that we're only creating one context at a time. This serves two purposes:
        //
        // 1. CGLChoosePixelFormat fails, returning `kCGLBadConnection`, if multiple threads try to
        //    open a display connection simultaneously.
        // 2. The first thread to create a context needs to load the GL function pointers.
        let mut previous_context_created = CREATE_CONTEXT_MUTEX.lock().unwrap();

        unsafe {
            let mut cgl_context = ptr::null_mut();
            let err = CGLCreateContext(color_surface.descriptor.cgl_pixel_format,
                                       ptr::null_mut(),
                                       &mut cgl_context);
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
                descriptor: color_surface.descriptor.clone(),
                framebuffer: Framebuffer::None,
                gl_info: GLInfo::new(),
            };

            if !*previous_context_created {
                gl::load_with(|symbol| {
                    self.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
                });
                *previous_context_created = true;
            }

            context.gl_info.populate(&self.context_descriptor_attributes(&context.descriptor));

            // Build the initial framebuffer.
            self.create_framebuffer(&mut context, color_surface)?;

            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<Option<Surface>, Error> {
        let mut result = Ok(None);
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

            result = self.destroy_surface_texture(context, color_surface_texture).map(Some);
        }

        unsafe {
            context.native_context.destroy();
        }

        result
    }

    #[inline]
    pub fn context_descriptor<'c>(&self, context: &'c Context) -> &'c ContextDescriptor {
        &context.descriptor
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
    pub fn context_color_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Object { ref color_surface_texture, .. } => {
                Ok(&color_surface_texture.surface)
            }
        }
    }

    pub fn replace_context_color_surface(&self, context: &mut Context, new_color_surface: Surface)
                                         -> Result<Surface, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget);
        }

        if new_color_surface.descriptor.cgl_pixel_format != context.descriptor.cgl_pixel_format {
            return Err(Error::IncompatibleContextDescriptor);
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
                color_surface_texture.surface().size() == new_color_surface.size()
            }
            Framebuffer::None | Framebuffer::External => unreachable!(),
        };
        if can_modify_existing_framebuffer {
            return self.replace_color_surface_in_existing_framebuffer(context, new_color_surface);
        }

        let old_surface = self.destroy_framebuffer(context)?;
        if let Err(err) = self.create_framebuffer(context, new_color_surface) {
            drop(self.destroy_surface(old_surface));
            return Err(err);
        }

        Ok(old_surface)
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Object { framebuffer_object, .. } => Ok(framebuffer_object),
        }
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        unsafe {
            let alpha_size = get_pixel_format_attribute(context_descriptor, kCGLPFAAlphaSize);
            let depth_size = get_pixel_format_attribute(context_descriptor, kCGLPFADepthSize);
            let stencil_size = get_pixel_format_attribute(context_descriptor, kCGLPFAStencilSize);
            let gl_profile = get_pixel_format_attribute(context_descriptor, kCGLPFAOpenGLProfile);

            let mut attribute_flags = ContextAttributeFlags::empty();
            attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
            attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
            attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

            let version = GLVersion::new(((gl_profile >> 12) & 0xf) as u8,
                                        ((gl_profile >> 8) & 0xf) as u8);

            return ContextAttributes {
                flags: attribute_flags,
                flavor: GLFlavor { api: GLApi::GL, version },
            };
        }

        unsafe fn get_pixel_format_attribute(context_descriptor: &ContextDescriptor,
                                             attribute: CGLPixelFormatAttribute)
                                             -> i32 {
            let mut value = 0;
            let err = CGLDescribePixelFormat(context_descriptor.cgl_pixel_format,
                                             0,
                                             attribute,
                                             &mut value);
            debug_assert_eq!(err, kCGLNoError);
            value
        }
    }

    // Assumes that the context is current.
    fn create_framebuffer(&self, context: &mut Context, color_surface: Surface)
                          -> Result<(), Error> {
        let size = color_surface.size();
        let color_surface_texture = self.create_surface_texture(context, color_surface)?;

        let context_attributes = self.context_descriptor_attributes(&context.descriptor);

        unsafe {
            let mut framebuffer_object = 0;
            gl::GenFramebuffers(1, &mut framebuffer_object);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     SurfaceTexture::gl_texture_target(),
                                     color_surface_texture.gl_texture(),
                                     0);

            let renderbuffers = Renderbuffers::new(&size, &context_attributes, &context.gl_info);
            renderbuffers.bind_to_current_framebuffer();

            debug_assert_eq!(gl::CheckFramebufferStatus(gl::FRAMEBUFFER),
                             gl::FRAMEBUFFER_COMPLETE);

            // Set the viewport so that the application doesn't have to do so explicitly.
            gl::Viewport(0, 0, size.width, size.height);

            context.framebuffer = Framebuffer::Object {
                framebuffer_object,
                color_surface_texture,
                renderbuffers,
            };
        }

        Ok(())
    }

    fn destroy_framebuffer(&self, context: &mut Context) -> Result<Surface, Error> {
        let (framebuffer_object,
             color_surface_texture,
             mut renderbuffers) = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::None | Framebuffer::External => unreachable!(),
            Framebuffer::Object { framebuffer_object, color_surface_texture, renderbuffers } => {
                (framebuffer_object, color_surface_texture, renderbuffers)
            }
        };

        let old_surface = self.destroy_surface_texture(context, color_surface_texture)?;
        renderbuffers.destroy();

        unsafe {
            gl::DeleteFramebuffers(1, &framebuffer_object);
        }

        Ok(old_surface)
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
