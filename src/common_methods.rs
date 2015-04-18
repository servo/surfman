use geom::Size2D;
use gleam::gl;
use GLContext;
use GLContextAttributes;
use DrawBuffer;

pub trait GLContextMethods {
    // TODO(ecoal95): create_headless should not require a size
    fn create_headless(Size2D<i32>) -> Result<Self, &'static str>;
    fn create_offscreen(Size2D<i32>, GLContextAttributes) -> Result<Self, &'static str>;
    fn make_current(&self) -> Result<(), &'static str>;
}

impl GLContext {
    // FIXME(ecoal95): resizing should be handled here
    pub fn init_offscreen(&mut self, size: Size2D<i32>, _: GLContextAttributes) -> Result<(), &'static str> {
        try!(self.create_draw_buffer(size));

        self.make_current().unwrap();

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Scissor(0, 0, size.width, size.height);
            gl::Viewport(0, 0, size.width, size.height);
        }

        Ok(())
    }

    pub fn create_draw_buffer(&mut self, size: Size2D<i32>) -> Result<(), &'static str> {
        self.draw_buffer = Some(try!(DrawBuffer::new(&self, size)));
        Ok(())
    }

    // Screen buffer is an abstraction over a framebuffer
    // attached to a native shared surface
    // fn create_screen_buffer(&self, width: usize, height: usize) {
    //     self.screen_buffer = Some(&mut GLScreenBuffer::new(&self, size));
    // }
}
