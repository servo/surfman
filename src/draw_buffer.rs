use geom::Size2D;
use gleam::gl;
use gleam::gl::types::{GLuint, GLsizei, GLenum};

use GLContext;
use GLContextAttributes;

/// This structure represents an offscreen context
/// draw buffer. It may have a
pub struct DrawBuffer {
    size: Size2D<i32>,
    framebuffer: GLuint,
    stencil_render_buffer: GLuint,
    depth_render_buffer: GLuint,
    color_render_buffer: GLuint,
    // samples: GLsizei,
}

/// Helper function to create a render buffer
/// TODO(ecoal95): We'll need to switch between `glRenderbufferStorage` and
///   `glRenderbufferStorageMultisample` when we support antialising
fn create_render_buffer(format: GLenum, size: &Size2D<i32>) -> GLuint {
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

        let attrs = &context.attributes;

        if attrs.antialias && context.capabilities.max_samples == 0 {
            return Err("The given GLContext doesn't support requested antialising");
        }

        let mut draw_buffer = DrawBuffer {
            size: size,
            framebuffer: 0,
            color_render_buffer: 0,
            stencil_render_buffer: 0,
            depth_render_buffer: 0,
            // samples: 0,
        };

        try!(draw_buffer.init(&attrs));

        unsafe {
            debug_assert!(gl::GetError() == gl::NO_ERROR);
        }

        Ok(draw_buffer)
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

            let mut render_buffers = [
                self.color_render_buffer,
                self.stencil_render_buffer,
                self.depth_render_buffer
            ];

            gl::DeleteRenderbuffers(3, render_buffers.as_mut_ptr());
        }
    }
}

trait DrawBufferHelpers {
    fn init(&mut self, attrs: &GLContextAttributes)   -> Result<(), &'static str>;
    fn attach_renderbuffers_to_framebuffer(&mut self) -> Result<(), &'static str>;
}

impl DrawBufferHelpers for DrawBuffer {
    fn init(&mut self, attrs: &GLContextAttributes) -> Result<(), &'static str> {
        // First we try to generate the framebuffer and the color buffer
        unsafe {
            gl::GenFramebuffers(1, &mut self.framebuffer);
        }
        debug_assert!(self.framebuffer != 0);

        if attrs.alpha {
            self.color_render_buffer = create_render_buffer(gl::RGBA8, &self.size);
        }

        // After this we check if we need stencil and depth buffers
        if attrs.depth {
            self.depth_render_buffer = create_render_buffer(gl::DEPTH_COMPONENT16, &self.size);
        }

        if attrs.stencil {
            self.stencil_render_buffer = create_render_buffer(gl::STENCIL_INDEX8, &self.size);
        }

        self.attach_renderbuffers_to_framebuffer()
    }

    fn attach_renderbuffers_to_framebuffer(&mut self) -> Result<(), &'static str> {
        unsafe {
            debug_assert!(gl::IsFramebuffer(self.framebuffer) != 0);
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.framebuffer);

            if self.color_render_buffer != 0 {
                debug_assert!(gl::IsRenderbuffer(self.color_render_buffer) != 0);
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                            gl::COLOR_ATTACHMENT0,
                                            gl::RENDERBUFFER,
                                            self.color_render_buffer);
            }

            if self.depth_render_buffer != 0 {
                debug_assert!(gl::IsRenderbuffer(self.depth_render_buffer) != 0);
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                            gl::DEPTH_ATTACHMENT,
                                            gl::RENDERBUFFER,
                                            self.depth_render_buffer);
            }

            if self.stencil_render_buffer != 0 {
                debug_assert!(gl::IsRenderbuffer(self.stencil_render_buffer) != 0);
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER,
                                            gl::STENCIL_ATTACHMENT,
                                            gl::RENDERBUFFER,
                                            self.stencil_render_buffer);
            }
        }

        Ok(())
    }
}
