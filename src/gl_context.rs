use geom::Size2D;
use gleam::gl;
use gleam::gl::types::{GLint, GLuint};

use NativeGLContextMethods;
use GLContextAttributes;
use GLContextCapabilities;
use GLFormats;
use DrawBuffer;
use ColorAttachmentType;
use NativeGLContext;

#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeGraphicsMetadata;


/// This is a wrapper over a native headless GL context
pub struct GLContext {
    native_context: NativeGLContext,
    /// This an abstraction over a custom framebuffer
    /// with attachments according to WebGLContextAttributes
    // TODO(ecoal95): Ideally we may want a read and a draw
    // framebuffer, but this is not supported in GLES2, review
    // when we have better support
    draw_buffer: Option<DrawBuffer>,
    attributes: GLContextAttributes,
    capabilities: GLContextCapabilities,
    formats: GLFormats,
}

impl GLContext {
    pub fn create_headless() -> Result<GLContext, &'static str> {
        let native_context = try!(NativeGLContext::create_headless());

        try!(native_context.make_current());

        let attributes = GLContextAttributes::any();
        let formats = GLFormats::detect(&attributes);

        Ok(GLContext {
            native_context: native_context,
            draw_buffer: None,
            attributes: attributes,
            capabilities: GLContextCapabilities::detect(),
            formats: formats,
        })
    }

    /// This allows to choose a color attachment type
    /// create_offscreen() chooses the default one
    pub fn create_offscreen_with_color_attachment(size: Size2D<i32>,
                                                  attributes: GLContextAttributes,
                                                  color_attachment_type: ColorAttachmentType)
        -> Result<GLContext, &'static str> {
        // We create a headless context with a dummy size, we're painting to the
        // draw_buffer's framebuffer anyways.
        let mut context = try!(GLContext::create_headless());

        context.formats = GLFormats::detect(&attributes);
        context.attributes = attributes;

        try!(context.init_offscreen(size, color_attachment_type));

        Ok(context)
    }

    #[inline(always)]
    pub fn create_offscreen(size: Size2D<i32>, attributes: GLContextAttributes)
        -> Result<GLContext, &'static str> {
        GLContext::create_offscreen_with_color_attachment(size, attributes, ColorAttachmentType::default())
    }

    #[inline(always)]
    pub fn make_current(&self) -> Result<(), &'static str> {
        self.native_context.make_current()
    }

    #[inline(always)]
    pub fn is_current(&self) -> bool {
        self.native_context.is_current()
    }

    // Allow borrowing these unmutably
    pub fn borrow_attributes(&self) -> &GLContextAttributes {
        &self.attributes
    }

    pub fn borrow_capabilities(&self) -> &GLContextCapabilities {
        &self.capabilities
    }

    pub fn borrow_formats(&self) -> &GLFormats {
        &self.formats
    }

    pub fn borrow_draw_buffer(&self) -> Option<&DrawBuffer> {
        self.draw_buffer.as_ref()
    }

    pub fn get_framebuffer(&self) -> GLuint {
        if let Some(ref db) = self.draw_buffer {
            return db.get_framebuffer();
        }

        unsafe {
            let mut ret : GLint = 0;
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut ret);
            ret as GLuint
        }
    }

    pub fn draw_buffer_size(&self) -> Option<Size2D<i32>> {
        self.draw_buffer.as_ref().map( |db| db.size() )
    }

    // We resize just replacing the draw buffer, we don't perform size optimizations
    // in order to keep this generic
    pub fn resize(&mut self, size: Size2D<i32>) -> Result<(), &'static str> {
        if self.draw_buffer.is_some() {
            let color_attachment_type = self.borrow_draw_buffer().unwrap().color_attachment_type();
            self.create_draw_buffer(size, color_attachment_type)
        } else {
            Err("No DrawBuffer found")
        }
    }

    #[cfg(feature="texture_surface")]
    pub fn get_metadata(&self) -> NativeGraphicsMetadata {
        self.native_context.get_metadata()
    }
}


trait GLContextPrivateMethods {
    fn init_offscreen(&mut self, Size2D<i32>, ColorAttachmentType) -> Result<(), &'static str>;
    fn create_draw_buffer(&mut self, Size2D<i32>, ColorAttachmentType) -> Result<(), &'static str>;
}

impl GLContextPrivateMethods for GLContext {
    fn init_offscreen(&mut self, size: Size2D<i32>, color_attachment_type: ColorAttachmentType) -> Result<(), &'static str> {
        try!(self.create_draw_buffer(size, color_attachment_type));

        debug_assert!(self.is_current());

        unsafe {
            gl::Scissor(0, 0, size.width, size.height);
            gl::Viewport(0, 0, size.width, size.height);
        }

        Ok(())
    }

    fn create_draw_buffer(&mut self, size: Size2D<i32>, color_attachment_type: ColorAttachmentType) -> Result<(), &'static str> {
        self.draw_buffer = Some(try!(DrawBuffer::new(&self, size, color_attachment_type)));
        Ok(())
    }
}
