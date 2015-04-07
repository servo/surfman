use glx;
use libc::*;
use xlib::*;
use gleam::gl;
use glx::types::{GLXContext, GLXDrawable, GLXFBConfig, GLXPixmap};
use geom::{Size2D};
use platform::glx::utils::{create_offscreen_pixmap_backed_context};
use GLContextMethods;
use gl_screen_buffer::{GLScreenBuffer};

pub struct GLContext {
    native_context: GLXContext,
    native_display: *mut glx::types::Display,
    native_drawable: GLXDrawable,
    delete_drawable_on_drop: bool,
    is_offscreen: bool,
    // screen_buffer: Option<&mut GLScreenBuffer>
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
            is_offscreen: is_offscreen,
            // screen_buffer: None
        })
    }

    fn as_native_glx_context(&self) -> GLXContext {
        self.native_context
    }

    // NOTE: This won't work until we have a correct screen buffer interpretation
    fn init_offscreen(&self, size: Size2D<i32>) -> Result<(), &'static str> {
        // try!(self.create_screen_buffer(&size));

        self.make_current();

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Scissor(0, 0, size.width, size.height);
            gl::Viewport(0, 0, size.width, size.height);
        }

        Ok(())
    }

    // Screen buffer is an abstraction over a framebuffer
    // attached to a native shared surface
    // fn create_screen_buffer(&self, width: usize, height: usize) {
    //     self.screen_buffer = Some(&mut GLScreenBuffer::new(&self, size));
    // }
}

impl Drop for GLContext {
    fn drop(&mut self) {
        self.make_current().unwrap();
        unsafe { glx::DestroyContext(self.native_display, self.native_context) };
        if self.delete_drawable_on_drop {
            unsafe { glx::DestroyPixmap(self.native_display, self.native_drawable as GLXPixmap); };
        }
    }
}

impl GLContextMethods for GLContext {
    fn create_headless() -> Result<GLContext, &'static str> {
        // 16, 16 => dummy size
        create_offscreen_pixmap_backed_context(16, 16)
    }

    fn create_offscreen(size: Size2D<i32>) -> Result<GLContext, &'static str> {
        let context = try!(GLContext::create_headless());
        context.init_offscreen(size).unwrap();

        Ok(context)
    }


    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if glx::GetCurrentContext() != self.native_context &&
               glx::MakeCurrent(self.native_display,
                                self.native_drawable,
                                self.native_context) == 0 {
                Err("glx::MakeContextCurrent")
            } else {
                Ok(())
            }
        }
    }
}

