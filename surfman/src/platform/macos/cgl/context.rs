// surfman/surfman/src/platform/macos/cgl/context.rs
//
//! Wrapper for Core OpenGL contexts.

use super::device::Device;
use super::error::ToWindowingApiError;
use super::ffi::{CGLReleaseContext, CGLRetainContext};
use super::surface::Surface;
use crate::context::{ContextID, CREATE_CONTEXT_MUTEX};
use crate::gl_utils;
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion, Gl, SurfaceInfo};

use cgl::{kCGLPFAAllowOfflineRenderers, kCGLPFAAlphaSize, kCGLPFADepthSize};
use cgl::{kCGLPFAOpenGLProfile, kCGLPFAStencilSize};
use cgl::{
    CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDescribePixelFormat, CGLError,
};
use cgl::{CGLGetCurrentContext, CGLGetPixelFormat, CGLPixelFormatAttribute, CGLPixelFormatObj};
use cgl::{CGLReleasePixelFormat, CGLRetainPixelFormat, CGLSetCurrentContext};
use core_foundation::base::TCFType;
use core_foundation::bundle::CFBundleGetBundleWithIdentifier;
use core_foundation::bundle::CFBundleGetFunctionPointerForName;
use core_foundation::bundle::CFBundleRef;
use core_foundation::string::CFString;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::str::FromStr;
use std::thread;

// No CGL error occurred.
#[allow(non_upper_case_globals)]
const kCGLNoError: CGLError = 0;

// Choose a renderer compatible with GL 1.0.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_Legacy: CGLPixelFormatAttribute = 0x1000;
// Choose a renderer capable of GL3.2 or later.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_3_2_Core: CGLPixelFormatAttribute = 0x3200;
// Choose a renderer capable of GL4.1 or later.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_GL4_Core: CGLPixelFormatAttribute = 0x4100;

static OPENGL_FRAMEWORK_IDENTIFIER: &'static str = "com.apple.opengl";

thread_local! {
    #[doc(hidden)]
    pub static GL_FUNCTIONS: Gl = Gl::load_with(get_proc_address);
}

thread_local! {
    static OPENGL_FRAMEWORK: CFBundleRef = {
        unsafe {
            let framework_identifier: CFString =
                FromStr::from_str(OPENGL_FRAMEWORK_IDENTIFIER).unwrap();
            let framework =
                CFBundleGetBundleWithIdentifier(framework_identifier.as_concrete_TypeRef());
            assert!(!framework.is_null());
            framework
        }
    };
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
    pub(crate) cgl_context: CGLContextObj,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<Surface, ()>,
}

/// Wraps a native CGL context object.
pub struct NativeContext(pub CGLContextObj);

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.cgl_context.is_null() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

/// Options that control OpenGL rendering.
///
/// This corresponds to a "pixel format" object in many APIs. These are thread-safe.
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
    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        if attributes
            .flags
            .contains(ContextAttributeFlags::COMPATIBILITY_PROFILE)
            && attributes.version.major > 2
        {
            return Err(Error::UnsupportedGLProfile);
        };

        let profile = if attributes.version.major >= 4 {
            kCGLOGLPVersion_GL4_Core
        } else if attributes.version.major == 3 {
            kCGLOGLPVersion_3_2_Core
        } else {
            kCGLOGLPVersion_Legacy
        };

        let flags = attributes.flags;
        let alpha_size = if flags.contains(ContextAttributeFlags::ALPHA) {
            8
        } else {
            0
        };
        let depth_size = if flags.contains(ContextAttributeFlags::DEPTH) {
            24
        } else {
            0
        };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) {
            8
        } else {
            0
        };

        let mut cgl_pixel_format_attributes = vec![
            kCGLPFAOpenGLProfile,
            profile,
            kCGLPFAAlphaSize,
            alpha_size,
            kCGLPFADepthSize,
            depth_size,
            kCGLPFAStencilSize,
            stencil_size,
        ];

        // This means "opt into the integrated GPU".
        //
        // https://supermegaultragroovy.com/2016/12/10/auto-graphics-switching/
        if self.adapter().0.is_low_power {
            cgl_pixel_format_attributes.push(kCGLPFAAllowOfflineRenderers);
        }

        cgl_pixel_format_attributes.extend_from_slice(&[0, 0]);

        unsafe {
            let (mut cgl_pixel_format, mut cgl_pixel_format_count) = (ptr::null_mut(), 0);
            let err = CGLChoosePixelFormat(
                cgl_pixel_format_attributes.as_ptr(),
                &mut cgl_pixel_format,
                &mut cgl_pixel_format_count,
            );
            if err != kCGLNoError {
                return Err(Error::PixelFormatSelectionFailed(
                    err.to_windowing_api_error(),
                ));
            }
            if cgl_pixel_format_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            Ok(ContextDescriptor { cgl_pixel_format })
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
        // Take a lock so that we're only creating one context at a time. `CGLChoosePixelFormat`
        // will fail, returning `kCGLBadConnection`, if multiple threads try to open a display
        // connection simultaneously.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        unsafe {
            // Create the CGL context.
            let mut cgl_context = ptr::null_mut();
            let err = CGLCreateContext(
                descriptor.cgl_pixel_format,
                share_with.map_or(ptr::null_mut(), |ctx| ctx.cgl_context),
                &mut cgl_context,
            );
            if err != kCGLNoError {
                return Err(Error::ContextCreationFailed(err.to_windowing_api_error()));
            }
            debug_assert_ne!(cgl_context, ptr::null_mut());

            // Wrap and return the context.
            let context = Context {
                cgl_context,
                id: *next_context_id,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;
            Ok(context)
        }
    }

    /// Wraps a `CGLContext` in a `surfman` context and returns it.
    ///
    /// This function takes ownership of the native context and does not adjust its reference
    /// count.
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext,
    ) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let context = Context {
            cgl_context: native_context.0,
            id: *next_context_id,
            framebuffer: Framebuffer::None,
        };
        next_context_id.0 += 1;
        mem::forget(native_context);
        Ok(context)
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.cgl_context.is_null() {
            return Ok(());
        }

        if let Framebuffer::Surface(mut surface) =
            mem::replace(&mut context.framebuffer, Framebuffer::None)
        {
            self.destroy_surface(context, &mut surface)?;
        }

        unsafe {
            CGLSetCurrentContext(ptr::null_mut());
            CGLReleaseContext(context.cgl_context);
            context.cgl_context = ptr::null_mut();
        }

        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let mut cgl_pixel_format = CGLGetPixelFormat(context.cgl_context);
            cgl_pixel_format = CGLRetainPixelFormat(cgl_pixel_format);
            ContextDescriptor { cgl_pixel_format }
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(context.cgl_context);
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
        }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(ptr::null_mut());
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
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
        match context.framebuffer {
            Framebuffer::External(_) => return Err((Error::ExternalRenderTarget, new_surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, new_surface)),
            Framebuffer::None => {}
        }

        if new_surface.context_id != context.id {
            return Err((Error::IncompatibleSurface, new_surface));
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
            Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
            Framebuffer::None | Framebuffer::Surface(_) => {}
        }

        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::External(_) => unreachable!(),
            Framebuffer::None => Ok(None),
            Framebuffer::Surface(surface) => {
                // Make sure all changes are synchronized. Apple requires this.
                //
                // TODO(pcwalton): Use `glClientWaitSync` instead to avoid starving the window
                // server.
                GL_FUNCTIONS.with(|gl| {
                    let _guard = self.temporarily_make_context_current(context)?;
                    unsafe {
                        gl.Flush();
                    }

                    gl_utils::unbind_framebuffer_if_necessary(gl, surface.framebuffer_object);
                    Ok(Some(surface))
                })
            }
        }
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        unsafe {
            let alpha_size = get_pixel_format_attribute(context_descriptor, kCGLPFAAlphaSize);
            let depth_size = get_pixel_format_attribute(context_descriptor, kCGLPFADepthSize);
            let stencil_size = get_pixel_format_attribute(context_descriptor, kCGLPFAStencilSize);
            let gl_profile = get_pixel_format_attribute(context_descriptor, kCGLPFAOpenGLProfile);

            let mut attribute_flags = ContextAttributeFlags::empty();
            attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
            attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
            attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

            let mut version = GLVersion::new(
                ((gl_profile >> 12) & 0xf) as u8,
                ((gl_profile >> 8) & 0xf) as u8,
            );
            if version.major <= 2 {
                version.major = 2;
                version.minor = 1;
                attribute_flags.insert(ContextAttributeFlags::COMPATIBILITY_PROFILE);
            }

            return ContextAttributes {
                flags: attribute_flags,
                version,
            };
        }

        unsafe fn get_pixel_format_attribute(
            context_descriptor: &ContextDescriptor,
            attribute: CGLPixelFormatAttribute,
        ) -> i32 {
            let mut value = 0;
            let err = CGLDescribePixelFormat(
                context_descriptor.cgl_pixel_format,
                0,
                attribute,
                &mut value,
            );
            debug_assert_eq!(err, kCGLNoError);
            value
        }
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
        get_proc_address(symbol_name)
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External(_) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }

    /// Given a context, returns its underlying CGL context object.
    ///
    /// The reference count on that context is incremented via `CGLRetainContext()` before
    /// returning.
    #[inline]
    pub fn native_context(&self, context: &Context) -> NativeContext {
        unsafe { NativeContext(CGLRetainContext(context.cgl_context)) }
    }
}

fn get_proc_address(symbol_name: &str) -> *const c_void {
    OPENGL_FRAMEWORK.with(|framework| unsafe {
        let symbol_name: CFString = FromStr::from_str(symbol_name).unwrap();
        CFBundleGetFunctionPointerForName(*framework, symbol_name.as_concrete_TypeRef())
    })
}

#[must_use]
pub(crate) struct CurrentContextGuard {
    old_cgl_context: CGLContextObj,
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        unsafe {
            CGLSetCurrentContext(self.old_cgl_context);
        }
    }
}

impl CurrentContextGuard {
    fn new() -> CurrentContextGuard {
        unsafe {
            CurrentContextGuard {
                old_cgl_context: CGLGetCurrentContext(),
            }
        }
    }
}

impl Clone for NativeContext {
    #[inline]
    fn clone(&self) -> NativeContext {
        unsafe { NativeContext(CGLRetainContext(self.0)) }
    }
}

impl Drop for NativeContext {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            CGLReleaseContext(self.0);
            self.0 = ptr::null_mut();
        }
    }
}

impl NativeContext {
    /// Returns the current context, wrapped as a `NativeContext`.
    ///
    /// If there is no current context, this returns a `NoCurrentContext` error.
    #[inline]
    pub fn current() -> Result<NativeContext, Error> {
        unsafe {
            let cgl_context = CGLGetCurrentContext();
            if !cgl_context.is_null() {
                Ok(NativeContext(cgl_context))
            } else {
                Err(Error::NoCurrentContext)
            }
        }
    }
}
