use GLContextMethods;
use platform::glx::utils::{create_offscreen_pixmap_backed_context};
use libc::*;
use xlib::*;
use glx;
use glx::types::{GLXContext, GLXDrawable, GLXFBConfig};

pub struct GLContext {
    native_context: GLXContext,
    native_display: *mut glx::types::Display,
    native_drawable: GLXDrawable,
    delete_drawable_on_drop: bool,
    is_offscreen: bool
}

impl GLContext {
    pub fn new(share_context: Option<&GLContext>,
               is_offscreen: bool,
               display: *mut glx::types::Display,
               drawable: GLXDrawable,
               framebuffer_config: GLXFBConfig,
               delete_drawable_on_drop: bool)
        -> Result<GLContext, &'static str> {

        let shared = match share_context {
            Some(ctx) => ctx.as_native_glx_context(),
            None      => 0 as GLXContext
        };

        let native = unsafe { glx::CreateNewContext(display, framebuffer_config, glx::RGBA_TYPE as c_int, shared, 1 as glx::types::Bool) };

        // FIXME: This should be:
        // if native == (0 as *const c_void) {
        // but that way compilation fails with error:
        //  expected `*const libc::types::common::c95::c_void`,
        //     found `*const libc::types::common::c95::c_void`
        // (expected enum `libc::types::common::c95::c_void`,
        //     found a different enum `libc::types::common::c95::c_void`) [E0308]
        if (native as *const c_void) == (0 as *const c_void) {
            return Err("Error creating native glx context");
        }

        Ok(GLContext {
            native_context: native,
            native_display: display,
            native_drawable: drawable,
            delete_drawable_on_drop: delete_drawable_on_drop,
            is_offscreen: is_offscreen
        })
    }

    fn as_native_glx_context(&self) -> GLXContext {
        self.native_context
    }
}

impl GLContextMethods for GLContext {
    fn create_headless() -> Result<GLContext, &'static str> {
        // 16, 16 => dummy size
        create_offscreen_pixmap_backed_context(16, 16)
    }

    fn create_offscreen() -> Result<GLContext, &'static str> {
        // TODO
        Err("Not implemented")
    }

    fn make_current(&self) -> Result<(), &'static str> {
        // TODO
        Err("Not implemented")
    }
}
