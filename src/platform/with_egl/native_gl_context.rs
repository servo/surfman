use crate::GLVersion;
use crate::egl::types::{EGLint, EGLBoolean, EGLDisplay, EGLSurface, EGLConfig, EGLContext};
use crate::egl;
use crate::gl_formats::Format;
use crate::platform::NativeGLContextMethods;
use crate::platform::with_egl::surface::{DISPLAY, NativeSurface};
use euclid::Size2D;
use gleam::gl;
use libloading as lib;
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::ffi::CString;
use std::ops::Deref;
use std::sync::Arc;

const DUMMY_FRAMEBUFFER_SIZE: i32 = 16;

lazy_static! {
    static ref GL_LIB: Option<lib::Library>  = {
        let names = if cfg!(target_os="windows") {
            &["libGLESv2.dll"][..]
        } else {
            &["libGLESv2.so", "libGL.so", "libGLESv3.so"][..]
        };
        for name in names {
            if let Ok(lib) = lib::Library::new(name) {
                return Some(lib)
            }
        }

        None
    };
}

thread_local! {
    static CURRENT_CONTEXT: RefCell<Option<NativeGLContext>> = RefCell::new(None);
}

struct GLContext {
    egl_context: EGLContext,
    default_surface: NativeSurface,
}

impl Drop for GLContext {
    fn drop(&mut self) {
        unsafe {
            // Unbind if necessary.
            if egl::GetCurrentContext() == self.egl_context {
                egl::MakeCurrent(DISPLAY.0,
                                 egl::NO_SURFACE as EGLSurface,
                                 egl::NO_SURFACE as EGLSurface,
                                 egl::NO_CONTEXT as EGLContext);
            }

            if egl::DestroyContext(DISPLAY.0, self.egl_context) == 0 {
                debug!("egl::DestroyContext failed");
            }
        }
    }
}

#[derive(Clone)]
pub struct NativeGLContext(Arc<GLContext>);

impl GLContext {
    fn new(default_surface: NativeSurface, share_context: Option<&EGLContext>)
           -> Result<GLContext, &'static str> {
        let shared = match share_context {
            Some(ctx) => *ctx,
            None => egl::NO_CONTEXT as EGLContext,
        };

        let client_version = default_surface.api_version().major_version() as EGLint;

        let attributes = [
            egl::CONTEXT_CLIENT_VERSION as EGLint, client_version,
            egl::NONE as EGLint, 0,
            0, 0, // see mod.rs
        ];

        let config = *default_surface.config();

        let mut ctx = unsafe {
            egl::CreateContext(DISPLAY.0, config, shared, attributes.as_ptr())
        };

        if share_context.is_some() && ctx == egl::NO_CONTEXT as EGLContext && client_version != 3 {
            // Workaround for GPUs that don't like different CONTEXT_CLIENT_VERSION value when sharing (e.g. Mali-T880).
            // Set CONTEXT_CLIENT_VERSION 3 to fix the shared ctx creation failure. Note that the ctx is still OpenGL ES 2.0
            // compliant because egl::OPENGL_ES2_BIT is set for egl::RENDERABLE_TYPE. See utils.rs.
            let attributes = [
                egl::CONTEXT_CLIENT_VERSION as EGLint, 3,
                egl::NONE as EGLint, 0, 0, 0, // see mod.rs
            ];
            ctx =  unsafe { egl::CreateContext(DISPLAY.0, config, shared, attributes.as_ptr()) };
        }

        // TODO: Check for every type of error possible, not just client error?
        // Note if we do it we must do it too on egl::CreatePBufferSurface, etc...
        if ctx == egl::NO_CONTEXT as EGLContext {
            return Err("Error creating an EGL context");
        }

        Ok(GLContext { egl_context: ctx, default_surface })
    }

}

impl NativeGLContext {
    #[inline]
    pub fn new(default_surface: NativeSurface, share_context: Option<&EGLContext>)
               -> Result<NativeGLContext, &'static str> {
        GLContext::new(default_surface, share_context).map(|context| {
            NativeGLContext(Arc::new(context))
        })
    }
}

pub type NativeGLContextHandle = NativeGLContext;

impl NativeGLContextMethods for NativeGLContext {
    type Handle = NativeGLContext;

    // According to the EGL spec <= 1.4, eglGetProcAddress should only be used to
    // retrieve extension functions. Some implementations return NULL for core OpenGL functions.
    // Other implementations may return non-NULL values even for invalid core or extension symbols.
    // This is very dangerous, so we use dlsym function before calling eglGetProcAddress
    // in order to avoid possible garbage pointers.
    fn get_proc_address(addr: &str) -> *const () {
        unsafe {
            if let Some(ref lib) = *GL_LIB {
                let symbol: Result<lib::Symbol<unsafe extern fn()>, _> = lib.get(addr.as_bytes());
                if let Ok(symbol) = symbol {
                    return *symbol.deref() as *const ();
                }
            }

            let addr = CString::new(addr.as_bytes());
            let addr = addr.unwrap().as_ptr();
            egl::GetProcAddress(addr) as *const ()
        }
    }

    fn create_headless(api_type: &gl::GlType, api_version: GLVersion)
                       -> Result<NativeGLContext, &'static str> {
        NativeGLContext::create_shared(None, api_type, api_version)
    }

    fn create_shared(with: Option<&Self::Handle>,
                     api_type: &gl::GlType,
                     api_version: GLVersion)
                     -> Result<NativeGLContext, &'static str> {
        let size = Size2D::new(DUMMY_FRAMEBUFFER_SIZE, DUMMY_FRAMEBUFFER_SIZE);
        let format = Format::RGBA;
        let surface = NativeSurface::from_version_size_format(*api_type,
                                                              api_version,
                                                              &size,
                                                              format);
        let native_share_context = with.map(|with| {
            &with.0.egl_context
        });

        NativeGLContext::new(surface, native_share_context)
    }

    #[inline]
    fn current_handle() -> Option<Self::Handle> {
        Self::current()
    }

    #[inline]
    fn is_current(&self) -> bool {
        CURRENT_CONTEXT.with(|current_context| {
            match *current_context.borrow() {
                None => false,
                Some(ref context) => context.0.egl_context == self.0.egl_context,
            }
        })
    }

    #[inline]
    fn current() -> Option<Self> {
        CURRENT_CONTEXT.with(|current_context| (*current_context.borrow()).as_ref().cloned())
    }

    fn make_current(&self) -> Result<(), &'static str> {
        if self.is_current() {
            return Ok(())
        }

        if let Some(old_context) = Self::current() {
            old_context.unbind();
        }

        unsafe {
            if egl::MakeCurrent(DISPLAY.0,
                                self.0.default_surface.egl_surface(),
                                self.0.default_surface.egl_surface(),
                                self.0.egl_context) == egl::FALSE as EGLBoolean {
                return Err("egl::MakeCurrent")
            }
        }

        CURRENT_CONTEXT.with(|current_context| {
            *current_context.borrow_mut() = Some((*self).clone())
        });

        Ok(())
    }

    fn handle(&self) -> Self::Handle {
        (*self).clone()
    }

    fn unbind(&self) -> Result<(), &'static str> {
        if !self.is_current() {
            return Ok(())
        }

        CURRENT_CONTEXT.with(|current_context| *current_context.borrow_mut() = None);

        unsafe {
           if egl::MakeCurrent(DISPLAY.0,
                               egl::NO_SURFACE as EGLSurface,
                               egl::NO_SURFACE as EGLSurface,
                               egl::NO_CONTEXT as EGLContext) == egl::FALSE as EGLBoolean {
                return Err("egl::MakeCurrent (on unbind)")
           }
        }

        Ok(())
    }
}
