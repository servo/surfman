use geom::Size2D;
use platform::NativeGLContextMethods;
use platform::with_egl::utils::{create_pixel_buffer_backed_offscreen_context};
use egl::egl::{self, EGLint, EGLDisplay, EGLSurface, EGLConfig, EGLContext};


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
            None => egl::EGL_NO_CONTEXT as EGLContext,
        };

        let attributes = [
            egl::EGL_CONTEXT_CLIENT_VERSION as EGLint, 2,
            egl::EGL_NONE as EGLint, 0, 0, 0, // see mod.rs
        ];

        let ctx = egl::CreateContext(display, config, shared, attributes.as_ptr());

        // TODO: Check for every type of error possible, not just client error?
        // Note if we do it we must do it too on egl::CreatePBufferSurface, etc...
        if ctx == (egl::EGL_NO_CONTEXT as EGLContext) {
            egl::DestroySurface(display, surface);
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


impl NativeGLContextMethods for NativeGLContext {
    fn get_proc_address(_addr: &str) -> *const () {
        // TODO: add eglGetProcAddress to rust-egl?
        // let addr = CString::new(addr.as_bytes()).unwrap().as_ptr();
        // egl::GetProcAddress(addr as *const _) as *const ()
        0 as *const ()
    }

    fn create_headless() -> Result<NativeGLContext, &'static str> {
        // We create a context with a dummy size, we can't rely on a
        // default framebuffer
        create_pixel_buffer_backed_offscreen_context(Size2D(16, 16))
    }

    #[inline(always)]
    fn is_current(&self) -> bool {
        egl::GetCurrentContext() == self.native_context
    }

    fn make_current(&self) -> Result<(), &'static str> {
        if !self.is_current() &&
            egl::MakeCurrent(self.native_display,
                             self.native_surface,
                             self.native_surface,
                             self.native_context) == egl::EGL_FALSE {
            Err("egl::MakeCurrent")
        } else {
            Ok(())
        }
    }
}
