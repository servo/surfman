//! Wrapper for Core OpenGL contexts.

use super::ffi::{CGLReleaseContext, CGLRetainContext};
use super::surface::Surface;
use crate::context::ContextID;
use crate::surface::Framebuffer;
use crate::{Error, Gl};

use cgl::CGLContextObj;
use cgl::{CGLGetCurrentContext, CGLPixelFormatObj};
use cgl::{CGLReleasePixelFormat, CGLRetainPixelFormat, CGLSetCurrentContext};
use std::ptr;
use std::rc::Rc;
use std::thread;

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
    pub(crate) framebuffer: Framebuffer<Surface, ()>,
    pub(crate) gl: Rc<Gl>,
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
    pub(crate) cgl_pixel_format: CGLPixelFormatObj,
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
    pub(crate) fn new() -> CurrentContextGuard {
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
