use geom::Size2D;
use gleam::gl;
use GLContext;
use GLContextAttributes;
use DrawBuffer;

/// Public interface
pub trait GLContextMethods {
    // TODO(ecoal95): create_headless should not require a size
    fn create_headless(Size2D<i32>) -> Result<Self, &'static str>;
    fn make_current(&self) -> Result<(), &'static str>;
    // This function implementation is platform-independent
    // fn create_offscreen(Size2D<i32>, GLContextAttributes) -> Result<GLContext, &'static str>;
}

impl GLContext {
    // This function implementation is platform-independent
    pub fn create_offscreen(size: Size2D<i32>, attrs: GLContextAttributes) -> Result<GLContext, &'static str> {
        let mut context = try!(GLContext::create_headless(size));
        context.attributes = attrs;

        try!(context.init_offscreen(size));

        Ok(context)
    }
}


trait GLContextPrivateMethods {
    fn init_offscreen(&mut self, Size2D<i32>) -> Result<(), &'static str>;
    fn create_draw_buffer(&mut self, Size2D<i32>) -> Result<(), &'static str>;
}

impl GLContextPrivateMethods for GLContext {
    // FIXME(ecoal95): resizing should be handled here
    fn init_offscreen(&mut self, size: Size2D<i32>) -> Result<(), &'static str> {
        try!(self.create_draw_buffer(size));

        self.make_current().unwrap();

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Scissor(0, 0, size.width, size.height);
            gl::Viewport(0, 0, size.width, size.height);
        }

        Ok(())
    }

    fn create_draw_buffer(&mut self, size: Size2D<i32>) -> Result<(), &'static str> {
        self.draw_buffer = Some(try!(DrawBuffer::new(&self, size)));
        Ok(())
    }
}
