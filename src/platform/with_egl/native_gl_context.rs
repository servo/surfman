use std::ffi::CString;
use geom::Size2D;
use NativeGLContextMethods;
use platform::with_egl::utils::{create_pixel_buffer_backed_offscreen_context};


pub struct NativeGLContext {
    native_surface: EGLSurface,
    native_config: EGLConfig,
    native_context: EGLContext,
    is_offscreen: bool
}

impl NativeGLContext {
    pub fn new(share_context: Option<&NativeGLContext>,
           is_offscreen: bool,
           surface: EGLSurface,
           config: EGLConfig)
        -> Result<NativeGLContext, &'static str> {
        let shared = match share_context {
            Some(ctx) => ctx.as_native_egl_context(),
            None => egl::NO_CONTEXT
        };

        let mut attributes = [
            egl::CONTEXT_CLIENT_VERSION, 2,
            egl_end_workarounding_bugs!()
        ];


        unsafe {
            let native = egl::CreateContext(egl::Display(), config, shared, attributes.as_mut_ptr());

            if native == 0 {
                egl::DestroySurface(surface);
                return Err("Error creating native EGL Context");
            }

            Ok(NativeGLContext {
                native_surface: surface,
                native_config: config,
                native_context: native,
                is_offscreen: is_offscreen
            })
        }
    }

    #[inline(always)]
    pub fn as_native_egl_context(&self) -> EGLContext {
        self.native_context
    }
}


impl NativeGLContextMethods for NativeGLContext {
    fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(s.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            egl::GetProcAddress(addr as *const _) as *const ()
        }
    }

    fn create_headless() -> Result<NativeGLContext, &'static str> {
        // We create a context with a dummy size, we can't rely on a
        // default framebuffer
        create_pixel_buffer_backed_offscreen_context(Size2D(16, 16))
    }

    fn create_offscreen(size: Size2D<i32>) -> Result<NativeGLContext, &'static str> {
        let context = try!(create_headless(size));

        try!(context.init_offscreen(size));

        Ok(())
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if egl::GetCurrentContext() != self.native_context &&
                egl::MakeCurrent(self.native_display,
                                 self.native_surface,
                                 self.native_surface,
                                 self.native_context) == egl::FALSE {
                Err("egl::MakeCurrent")
            } else {
                Ok(())
            }
        }
    }
}
