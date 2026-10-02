//! Wrapper for Core OpenGL contexts.

use super::ffi::{CGLReleaseContext, CGLRetainContext};
use crate::cgl::surface::CglSurface;
use crate::context::ContextID;
use crate::surface::Framebuffer;
use crate::{Error, Gl};

use cgl::CGLContextObj;
use cgl::{CGLGetCurrentContext, CGLPixelFormatObj};
use cgl::{CGLReleasePixelFormat, CGLRetainPixelFormat, CGLSetCurrentContext};
use std::ptr;
use std::rc::Rc;
use std::thread;

/// An implementation of [`crate::Context`] for CGL.
pub struct CglContext {
    pub(crate) cgl_context: CGLContextObj,
    pub(crate) id: ContextID,
    pub(crate) framebuffer: Framebuffer<CglSurface, ()>,
    pub(crate) gl: Rc<Gl>,
}

/// Wraps a native CGL context object.
pub struct NativeContext(pub CGLContextObj);

impl Drop for CglContext {
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
pub struct CglContextDescriptor {
    pub(crate) cgl_pixel_format: CGLPixelFormatObj,
}

impl Drop for CglContextDescriptor {
    // These have been verified to be thread-safe.
    #[inline]
    fn drop(&mut self) {
        unsafe {
            CGLReleasePixelFormat(self.cgl_pixel_format);
        }
    }
}

impl Clone for CglContextDescriptor {
    #[inline]
    fn clone(&self) -> CglContextDescriptor {
        unsafe {
            CglContextDescriptor {
                cgl_pixel_format: CGLRetainPixelFormat(self.cgl_pixel_format),
            }
        }
    }
}

unsafe impl Send for CglContextDescriptor {}

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
