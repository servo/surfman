use geom::Size2D;
use gleam::gl;
use gleam::gl::types::{GLuint, GLenum};

use GLContext;
use GLContextAttributes;
use GLFormats;

/// This structure represents an offscreen context
/// draw buffer. It has a framebuffer, with at least
/// color renderbuffer (alpha or not). It may also have
/// a depth or stencil buffer, depending on context
/// requirements.
pub struct DrawBuffer {
    size: Size2D<i32>,
    framebuffer: GLuint,
    stencil_renderbuffer: GLuint,
    depth_renderbuffer: GLuint,
    color_renderbuffer: GLuint,
    // samples: GLsizei,
}

/// Helper function to create a render buffer
/// TODO(ecoal95): We'll need to switch between `glRenderbufferStorage` and
///   `glRenderbufferStorageMultisample` when we support antialising
fn create_renderbuffer(format: GLenum, size: &Size2D<i32>) -> GLuint {
    let mut ret: GLuint = 0;

    unsafe {
        gl::GenRenderbuffers(1, &mut ret);
        gl::BindRenderbuffer(gl::RENDERBUFFER, ret);
        gl::RenderbufferStorage(gl::RENDERBUFFER, format, size.width, size.height);
    }

    ret
}

impl DrawBuffer {
    pub fn new(context: &GLContext, size: Size2D<i32>)
        -> Result<DrawBuffer, &'static str> {

        let attrs = context.borrow_attributes();
        let capabilities = context.borrow_capabilities();
        let formats = context.borrow_formats();

        if attrs.antialias && capabilities.max_samples == 0 {
            return Err("The given GLContext doesn't support requested antialising");
        }

        let mut draw_buffer = DrawBuffer {
            size: size,
            framebuffer: 0,
            color_renderbuffer: 0,
            stencil_renderbuffer: 0,
            depth_renderbuffer: 0,
            // samples: 0,
        };

        try!(context.make_current());

        try!(draw_buffer.init(&attrs, &formats));

        unsafe {
            debug_assert!(gl::GetError() == gl::NO_ERROR);
        }

        Ok(draw_buffer)
    }

    #[inline(always)]
    pub fn get_framebuffer(&self) -> GLuint {
        self.framebuffer
    }
}

// NOTE: The initially associated GLContext MUST be the current gl context
// when drop is called. I know this is an important constraint.
// Right now there are no problems, if not, consider using a pointer to a
// parent with Rc<GLContext> and call make_current()
impl Drop for DrawBuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &mut self.framebuffer);

            let mut renderbuffers = [
                self.color_renderbuffer,
                self.stencil_renderbuffer,
                self.depth_renderbuffer
            ];

            gl::DeleteRenderbuffers(3, renderbuffers.as_mut_ptr());
        }
    }
}

trait DrawBufferHelpers {
    fn init(&mut self, attrs: &GLContextAttributes, formats: &GLFormats)
        -> Result<(), &'static str>;
    fn attach_renderbuffers_to_framebuffer(&mut self)
        -> Result<(), &'static str>;
}

impl DrawBufferHelpers for DrawBuffer {
    fn init(&mut self, attrs: &GLContextAttributes, formats: &GLFormats) -> Result<(), &'static str> {
        self.color_renderbuffer = create_renderbuffer(formats.color_renderbuffer, &self.size);
        debug_assert!(self.color_renderbuffer != 0);

        // After this we check if we need stencil and depth buffers
        if attrs.depth {
            self.depth_renderbuffer = create_renderbuffer(formats.depth, &self.size);
            debug_assert!(self.depth_renderbuffer != 0);
        }

        if attrs.stencil {
            self.stencil_renderbuffer = create_renderbuffer(formats.stencil, &self.size);
            debug_assert!(self.stencil_renderbuffer != 0);
        }

        unsafe {
            gl::GenFramebuffers(1, &mut self.framebuffer);
            debug_assert!(self.framebuffer != 0);
        }

        // Finally we attach them to the framebuffer
        self.attach_renderbuffers_to_framebuffer()
    }

    fn attach_renderbuffers_to_framebuffer(&mut self) -> Result<(), &'static str> {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.framebuffer);
            // NOTE: The assertion fails if the framebuffer is not bound
            debug_assert!(gl::IsFramebuffer(self.framebuffer) == gl::TRUE);

            if self.color_renderbuffer != 0 {
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                            gl::COLOR_ATTACHMENT0,
                                            gl::RENDERBUFFER,
                                            self.color_renderbuffer);
                // debug_assert!(gl::IsRenderbuffer(self.color_renderbuffer) == gl::TRUE);
            }

            if self.depth_renderbuffer != 0 {
                // debug_assert!(gl::IsRenderbuffer(self.depth_renderbuffer) == gl::TRUE);
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                            gl::DEPTH_ATTACHMENT,
                                            gl::RENDERBUFFER,
                                            self.depth_renderbuffer);
            }

            if self.stencil_renderbuffer != 0 {
                // debug_assert!(gl::IsRenderbuffer(self.stencil_renderbuffer) == gl::TRUE);
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                            gl::STENCIL_ATTACHMENT,
                                            gl::RENDERBUFFER,
                                            self.stencil_renderbuffer);
            }
        }

        Ok(())
    }
}
