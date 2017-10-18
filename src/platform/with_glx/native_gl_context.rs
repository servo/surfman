use std::ffi::CString;

use gl_context::GLVersion;
use gleam::gl;
use glx;
use glx_extra;
use std::os::raw::*;
use glx::types::{GLXContext, GLXDrawable, GLXFBConfig, GLXPixmap};
use euclid::Size2D;
use super::utils::{create_offscreen_pixmap_backed_context};

use platform::NativeGLContextMethods;

pub struct NativeGLContextHandle(pub GLXContext, pub *mut glx::types::Display);

unsafe impl Send for NativeGLContextHandle {}

pub struct NativeGLContext {
    native_context: GLXContext,
    native_display: *mut glx::types::Display,
    native_drawable: GLXDrawable,
    weak: bool,
}

impl NativeGLContext {
    pub fn new(share_context: Option<&GLXContext>,
               api_version: GLVersion,
               display: *mut glx::types::Display,
               drawable: GLXDrawable,
               framebuffer_config: GLXFBConfig,
               extensions: String)
        -> Result<NativeGLContext, &'static str> {

        let shared = match share_context {
            Some(ctx) => *ctx,
            None      => 0 as GLXContext,
        };

        let native =  if extensions.split(' ').find(|&i| i == "GLX_ARB_create_context").is_some() {
            let (major, minor) = match api_version {
                GLVersion::Major(major) => { (major, 1) }, // OpenGL 2.1, 3.1
                GLVersion::MajorMinor(major, minor) => { (major, minor) }
            };

            let attributes = [
                glx_extra::CONTEXT_MAJOR_VERSION_ARB as c_int, major as c_int,
                glx_extra::CONTEXT_MINOR_VERSION_ARB as c_int, minor as c_int,
                0
            ];

            // load the extra GLX functions
            let extra_functions = glx_extra::Glx::load_with(|s| {
                let c_str = CString::new(s.as_bytes()).unwrap();
                unsafe { glx::GetProcAddress(c_str.as_ptr() as *const u8) as *const _ }
            });

            unsafe {
                extra_functions.CreateContextAttribsARB(display as *mut _,
                                                        framebuffer_config,
                                                        shared, 1 as glx::types::Bool,
                                                        attributes.as_ptr())
            }
        } else {
             unsafe { 
                 glx::CreateNewContext(display,
                                       framebuffer_config,
                                       glx::RGBA_TYPE as c_int,
                                       shared,
                                       1 as glx::types::Bool)
            }
        };

        if native.is_null() {
            unsafe { glx::DestroyPixmap(display, drawable as GLXPixmap) };
            return Err("Error creating native glx context");
        }

        Ok(NativeGLContext {
            native_context: native,
            native_display: display,
            native_drawable: drawable,
            weak: false,
        })
    }

    pub fn as_native_glx_context(&self) -> GLXContext {
        self.native_context
    }
}

impl Drop for NativeGLContext {
    fn drop(&mut self) {
        // Unbind the current context to free the resources
        // inmediately
        if !self.weak {
            let _ = self.unbind(); // We don't want to panic
            unsafe {
                glx::DestroyContext(self.native_display, self.native_context);
                glx::DestroyPixmap(self.native_display, self.native_drawable as GLXPixmap);
            }
        }
    }
}

impl NativeGLContextMethods for NativeGLContext {
    type Handle = NativeGLContextHandle;

    fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            glx::GetProcAddress(addr as *const _) as *const ()
        }
    }

    fn current_handle() -> Option<Self::Handle> {
        let current = unsafe { glx::GetCurrentContext() };
        let dpy = unsafe { glx::GetCurrentDisplay() };

        if current.is_null() || dpy.is_null() {
            None
        } else {
            Some(NativeGLContextHandle(current, dpy))
        }
    }

    fn current() -> Option<NativeGLContext> {
        if let Some(handle) = Self::current_handle() {
            unsafe {
                Some(NativeGLContext {
                    native_context: handle.0,
                    native_display: handle.1,
                    native_drawable: glx::GetCurrentDrawable(),
                    weak: true,
                })
            }
        } else {
            None
        }
    }

    fn create_shared(with: Option<&Self::Handle>,
                     api_type: &gl::GlType,
                     api_version: GLVersion) -> Result<NativeGLContext, &'static str> {
        create_offscreen_pixmap_backed_context(Size2D::new(16, 16), with, api_type, api_version)
    }

    #[inline(always)]
    fn is_current(&self) -> bool {
        unsafe {
            glx::GetCurrentContext() == self.native_context
        }
    }

    fn handle(&self) -> NativeGLContextHandle {
        NativeGLContextHandle(self.native_context, self.native_display)
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if !self.is_current() &&
               glx::MakeCurrent(self.native_display,
                                self.native_drawable,
                                self.native_context) == 0 {
                Err("glx::MakeCurrent")
            } else {
                Ok(())
            }
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        unsafe {
            if self.is_current() &&
               glx::MakeCurrent(self.native_display,
                                0 as GLXDrawable,
                                0 as GLXContext) == 0 {
                Err("glx::MakeCurrent (on unbind)")
            } else {
                Ok(())
            }
        }
    }
}
