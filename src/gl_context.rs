use geom::Size2D;
use gleam::gl;

use NativeGLContextMethods;
use GLContextAttributes;
use GLContextCapabilities;
use DrawBuffer;
use NativeGLContext;


pub struct GLContext {
    native_context: NativeGLContext,
    draw_buffer: Option<DrawBuffer>,
    attributes: GLContextAttributes,
    capabilities: GLContextCapabilities,
}

impl GLContext {
    pub fn create_headless(size: Size2D<i32>) -> Result<GLContext, &'static str> {
        let native_context = try!(NativeGLContext::create_headless(size));

        try!(native_context.make_current());

        Ok(GLContext {
            native_context: native_context,
            draw_buffer: None,
            attributes: GLContextAttributes::any(),
            capabilities: GLContextCapabilities::detect()
        })
    }

    pub fn create_offscreen(size: Size2D<i32>, attributes: GLContextAttributes) -> Result<GLContext, &'static str> {
        let mut context = try!(GLContext::create_headless(size));

        context.attributes = attributes;

        try!(context.init_offscreen(size));

        Ok(context)
    }

    #[inline(always)]
    pub fn make_current(&self) -> Result<(), &'static str> {
        self.native_context.make_current()
    }

    // Allow borrowing these unmutably
    pub fn borrow_attributes(&self) -> &GLContextAttributes {
        &self.attributes
    }

    pub fn borrow_capabilities(&self) -> &GLContextCapabilities {
        &self.capabilities
    }
}


trait GLContextPrivateMethods {
    fn init_offscreen(&mut self, Size2D<i32>) -> Result<(), &'static str>;
    fn create_draw_buffer(&mut self, Size2D<i32>) -> Result<(), &'static str>;
}

impl GLContextPrivateMethods for GLContext {
    // FIXME(ecoal95): initial resizing should be handled here,
    //   generic resizing should be handled in the screen buffer/draw buffer
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
