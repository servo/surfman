use euclid::Size2D;
use platform::NativeGLContextMethods;
use platform::with_egl::utils::{create_pixel_buffer_backed_offscreen_context};
use std::ffi::CString;
use egl;
use egl::types::{EGLint, EGLBoolean, EGLDisplay, EGLSurface, EGLConfig, EGLContext};

#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeDisplay;
#[cfg(feature="texture_surface")]
use std::mem;

pub struct NativeGLContext {
    native_display: EGLDisplay,
    native_surface: EGLSurface,
    native_context: EGLContext,
}

impl NativeGLContext {
    pub fn new(share_context: Option<&NativeGLContext>,
               display: EGLDisplay,
               surface: EGLSurface,
               config: EGLConfig)
        -> Result<NativeGLContext, &'static str> {

        let shared = match share_context {
            Some(ctx) => ctx.as_native_egl_context(),
            None => egl::NO_CONTEXT as EGLContext,
        };

        let attributes = [
            egl::CONTEXT_CLIENT_VERSION as EGLint, 2,
            egl::NONE as EGLint, 0, 0, 0, // see mod.rs
        ];

        let ctx =  unsafe { egl::CreateContext(display, config, shared, attributes.as_ptr()) };

        // TODO: Check for every type of error possible, not just client error?
        // Note if we do it we must do it too on egl::CreatePBufferSurface, etc...
        if ctx == (egl::NO_CONTEXT as EGLContext) {
            unsafe { egl::DestroySurface(display, surface) };
            return Err("Error creating an EGL context");
        }

        Ok(NativeGLContext {
            native_display: display,
            native_surface: surface,
            native_context: ctx,
        })
    }

    #[inline(always)]
    pub fn as_native_egl_context(&self) -> EGLContext {
        self.native_context
    }
}

impl Drop for NativeGLContext {
    fn drop(&mut self) {
        let _ = self.unbind();
        unsafe {
            if egl::DestroySurface(self.native_display, self.native_surface) == 0 {
                debug!("egl::DestroySurface failed");
            }
            if egl::DestroyContext(self.native_display, self.native_context) == 0 {
                debug!("egl::DestroyContext failed");
            }
        }
    }
}

impl NativeGLContextMethods for NativeGLContext {
    fn get_proc_address(addr: &str) -> *const () {
        unsafe {
            let addr = CString::new(addr.as_bytes()).unwrap().as_ptr();
            egl::GetProcAddress(addr as *const _) as *const ()
        }
    }

    fn create_headless() -> Result<NativeGLContext, &'static str> {
        // We create a context with a dummy size, we can't rely on a
        // default framebuffer
        create_pixel_buffer_backed_offscreen_context(Size2D::new(16, 16))
    }

    #[inline(always)]
    fn is_current(&self) -> bool {
        unsafe {
            egl::GetCurrentContext() == self.native_context
        }
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if !self.is_current() &&
                egl::MakeCurrent(self.native_display,
                                 self.native_surface,
                                 self.native_surface,
                                 self.native_context) == (egl::FALSE as EGLBoolean) {
                Err("egl::MakeCurrent")
            } else {
                Ok(())
            }
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        unsafe {
            if self.is_current() &&
               egl::MakeCurrent(self.native_display,
                                egl::NO_SURFACE as EGLSurface,
                                egl::NO_SURFACE as EGLSurface,
                                egl::NO_CONTEXT as EGLContext) == (egl::FALSE as EGLBoolean) {
                Err("egl::MakeCurrent (on unbind)")
            } else {
                Ok(())
            }
        }
    }

    #[cfg(feature="texture_surface")]
    fn get_display(&self) -> NativeDisplay {
        unsafe {
            // FIXME: https://github.com/servo/servo/pull/6423#issuecomment-113282933
            NativeDisplay::new_with_display(mem::transmute(self.native_display))
        }
    }
}
