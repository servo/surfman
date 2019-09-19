//! Wrapper for Core OpenGL contexts.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLInfo, GLVersion};
use super::adapter::Adapter;
use super::device::Device;
use super::error::ToWindowingApiError;
use super::surface::{Framebuffer, Surface};
use cgl::{CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDescribePixelFormat};
use cgl::{CGLDestroyContext, CGLError, CGLGetCurrentContext, CGLGetPixelFormat};
use cgl::{CGLPixelFormatAttribute, CGLPixelFormatObj, CGLReleasePixelFormat, CGLRetainPixelFormat};
use cgl::{CGLSetCurrentContext, kCGLPFAAlphaSize, kCGLPFADepthSize};
use cgl::{kCGLPFAStencilSize, kCGLPFAOpenGLProfile};
use euclid::default::Size2D;
use gl;
use gl::types::GLuint;
use std::mem;
use std::ptr;
use std::sync::Mutex;
use std::thread;

// No CGL error occurred.
#[allow(non_upper_case_globals)]
const kCGLNoError: CGLError = 0;

lazy_static! {
    static ref CREATE_CONTEXT_MUTEX: Mutex<ContextID> = Mutex::new(ContextID(0));
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

#[derive(Clone, Copy, PartialEq)]
pub struct ContextID(pub u64);

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    pub(crate) gl_info: GLInfo,
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

impl Device {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let profile = if attributes.version.major >= 4 {
            kCGLOGLPVersion_GL4_Core
        } else if attributes.version.major == 3 {
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
    /// query or replace the surface—e.g. `replace_context_surface`—will fail if called with a
    /// context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Grab the current context.
        let native_context = Box::new(UnsafeCGLContextRef::current());

        // Create the context.
        let mut context = Context {
            native_context,
            id: *next_context_id,
            gl_info: GLInfo::new(),
            framebuffer: Framebuffer::External,
        };
        next_context_id.0 += 1;

        let device = Device::new(&Adapter)?;

        let context_descriptor = device.context_descriptor(&context);
        let context_attributes = device.context_descriptor_attributes(&context_descriptor);
        context.gl_info.populate(&context_attributes);

        Ok((device, context))
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor, size: &Size2D<i32>)
                          -> Result<Context, Error> {
        // Take a lock so that we're only creating one context at a time. This serves two purposes:
        //
        // 1. CGLChoosePixelFormat fails, returning `kCGLBadConnection`, if multiple threads try to
        //    open a display connection simultaneously.
        // 2. The first thread to create a context needs to load the GL function pointers.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        unsafe {
            let mut cgl_context = ptr::null_mut();
            let err = CGLCreateContext(descriptor.cgl_pixel_format,
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

            let mut context = Context {
                native_context,
                id: *next_context_id,
                framebuffer: Framebuffer::None,
                gl_info: GLInfo::new(),
            };
            next_context_id.0 += 1;

            let context_descriptor = self.context_descriptor(&context);
            let context_attributes = self.context_descriptor_attributes(&context_descriptor);
            context.gl_info.populate(&context_attributes);

            // Build the initial framebuffer.
            context.framebuffer = Framebuffer::Surface(self.create_surface(&context, size)?);
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Framebuffer::Surface(surface) = mem::replace(&mut context.framebuffer,
                                                            Framebuffer::None) {
            self.destroy_surface(context, surface)?;
        }

        unsafe {
            context.native_context.destroy();
        }

        Ok(())
    }

    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let mut cgl_pixel_format = CGLGetPixelFormat(context.native_context.cgl_context());
            cgl_pixel_format = CGLRetainPixelFormat(cgl_pixel_format);
            ContextDescriptor { cgl_pixel_format }
        }
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

    #[inline]
    pub fn context_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget);
        }

        if new_surface.context_id != context.id {
            return Err(Error::IncompatibleSurface);
        }

        self.make_context_current(context)?;

        // Make sure all changes are synchronized. Apple requires this.
        unsafe {
            gl::Flush();
        }

        let new_framebuffer = Framebuffer::Surface(new_surface);
        match mem::replace(&mut context.framebuffer, new_framebuffer) {
            Framebuffer::None | Framebuffer::External => unreachable!(),
            Framebuffer::Surface(old_surface) => Ok(old_surface),
        }
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface.framebuffer_object),
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

            return ContextAttributes { flags: attribute_flags, version };
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
