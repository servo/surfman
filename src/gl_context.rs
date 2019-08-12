use euclid::default::Size2D;
use gleam::gl;
use gleam::gl::types::{GLuint};
use std::mem;
use std::rc::Rc;

use crate::NativeGLContextMethods;
use crate::GLContextAttributes;
use crate::GLContextCapabilities;
use crate::GLFormats;
use crate::GLLimits;
use crate::DrawBuffer;
use crate::ColorAttachmentType;
use crate::platform::{NativeSurface, NativeSurfaceTexture};

/// This is a wrapper over a native headless GL context
pub struct GLContext<Native> {
    gl_: Rc<dyn gl::Gl>,
    api_type: gl::GlType,
    api_version: GLVersion,
    native_context: Native,
    /// This an abstraction over a custom framebuffer
    /// with attachments according to WebGLContextAttributes
    // TODO(ecoal95): Ideally we may want a read and a draw
    // framebuffer, but this is not supported in GLES2, review
    // when we have better support
    draw_buffer: Option<DrawBuffer>,
    attributes: GLContextAttributes,
    capabilities: GLContextCapabilities,
    formats: GLFormats,
    limits: GLLimits,
    extensions: Vec<String>
}

impl<Native> GLContext<Native>
    where Native: NativeGLContextMethods,
{
    pub fn create(api_type: gl::GlType,
                  api_version: GLVersion,
                  shared_with: Option<&Native::Handle>)
                  -> Result<Self, &'static str> {
        Self::create_shared_with_dispatcher(&api_type, api_version, shared_with, None)
    }

    pub fn create_shared_with_dispatcher(api_type: &gl::GlType,
                                         api_version: GLVersion,
                                         shared_with: Option<&Native::Handle>,
                                         dispatcher: Option<Box<dyn GLContextDispatcher>>)
        -> Result<Self, &'static str> {
        let native_context = Native::create_shared_with_dispatcher(shared_with,
                                                                   api_type,
                                                                   api_version,
                                                                   dispatcher)?;
        let gl_ = match api_type {
            gl::GlType::Gl => unsafe { gl::GlFns::load_with(|s| Self::get_proc_address(s) as *const _) },
            gl::GlType::Gles => unsafe { gl::GlesFns::load_with(|s| Self::get_proc_address(s) as *const _) },
        };

        native_context.make_current()?;
        let extensions = Self::query_extensions(&gl_, api_version);
        let attributes = GLContextAttributes::any();
        let formats = GLFormats::detect(&attributes, &extensions[..], api_type, api_version);
        let limits = GLLimits::detect(&*gl_);

        Ok(GLContext {
            gl_: gl_,
            api_type: *api_type,
            api_version,
            native_context: native_context,
            draw_buffer: None,
            attributes: attributes,
            capabilities: GLContextCapabilities::detect(),
            formats: formats,
            limits: limits,
            extensions: extensions
        })
    }

    #[inline(always)]
    pub fn get_proc_address(addr: &str) -> *const () {
        Native::get_proc_address(addr)
    }

    #[inline(always)]
    pub fn current_handle() -> Option<Native::Handle> {
        Native::current_handle()
    }

    pub fn new(size: Size2D<i32>,
               attributes: GLContextAttributes,
               color_attachment_type: ColorAttachmentType,
               api_type: gl::GlType,
               api_version: GLVersion,
               shared_with: Option<&Native::Handle>)
        -> Result<Self, &'static str> {
        Self::new_shared_with_dispatcher(size,
                                         attributes,
                                         color_attachment_type,
                                         api_type,
                                         api_version,
                                         shared_with,
                                         None)
    }

    pub fn new_shared_with_dispatcher(size: Size2D<i32>,
                                      attributes: GLContextAttributes,
                                      color_attachment_type: ColorAttachmentType,
                                      api_type: gl::GlType,
                                      api_version: GLVersion,
                                      shared_with: Option<&Native::Handle>,
                                      dispatcher: Option<Box<dyn GLContextDispatcher>>)
        -> Result<Self, &'static str> {
        // We create a headless context with a dummy size, we're painting to the
        // draw_buffer's framebuffer anyways.
        let mut context =
            Self::create_shared_with_dispatcher(&api_type,
                                                api_version,
                                                shared_with,
                                                dispatcher)?;

        context.formats = GLFormats::detect(&attributes, &context.extensions[..], &api_type, api_version);
        context.attributes = attributes;

        context.init_offscreen(size, color_attachment_type)?;

        Ok(context)
    }

    #[inline(always)]
    pub fn with_default_color_attachment(size: Size2D<i32>,
                                         attributes: GLContextAttributes,
                                         api_type: gl::GlType,
                                         api_version: GLVersion,
                                         shared_with: Option<&Native::Handle>)
        -> Result<Self, &'static str> {
        Self::new(size, attributes, ColorAttachmentType::default(), api_type, api_version, shared_with)
    }

    #[inline(always)]
    pub fn make_current(&self) -> Result<(), &'static str> {
        self.native_context.make_current()
    }

    #[inline(always)]
    pub fn unbind(&self) -> Result<(), &'static str> {
        let ret = self.native_context.unbind();

        // OSMesa doesn't allow any API to unbind a context before [1], and just
        // bails out on null context, buffer, or whatever, so not much we can do
        // here. Thus, ignore the failure and just flush the context if we're
        // using an old OSMesa version.
        //
        // [1]: https://www.mail-archive.com/mesa-dev@lists.freedesktop.org/msg128408.html
        if self.native_context.is_osmesa() && ret.is_err() {
            self.gl().flush();
            return Ok(())
        }

        ret
    }

    #[inline(always)]
    pub fn is_current(&self) -> bool {
        self.native_context.is_current()
    }

    #[inline(always)]
    pub fn handle(&self) -> Native::Handle {
        self.native_context.handle()
    }

    pub fn gl(&self) -> &dyn gl::Gl {
        &*self.gl_
    }

    pub fn clone_gl(&self) -> Rc<dyn gl::Gl> {
        self.gl_.clone()
    }

    #[inline]
    pub fn api_type(&self) -> gl::GlType {
        self.api_type
    }

    #[inline]
    pub fn api_version(&self) -> GLVersion {
        self.api_version
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

    pub fn borrow_limits(&self) -> &GLLimits {
        &self.limits
    }

    #[inline]
    pub fn draw_buffer(&self) -> Option<&DrawBuffer> {
        self.draw_buffer.as_ref()
    }

    #[inline]
    pub fn swap_native_surface(&mut self, new_surface: Option<NativeSurface>)   
                               -> Option<NativeSurfaceTexture> {
        match self.draw_buffer {
            None => None,
            Some(ref mut draw_buffer) => Some(draw_buffer.swap_native_surface(new_surface)),
        }
    }

    /*
    pub fn back_framebuffer(&self) -> Option<GLuint> {
        self.back_buffer().map(|back_buffer| back_buffer.get_framebuffer())
    }

    pub fn draw_buffer_size(&self) -> Option<Size2D<i32>> {
        self.front_buffer().map(|buffer| buffer.size())
    }
    */

    // We resize just replacing the draw buffer, we don't perform size optimizations
    // in order to keep this generic. The old buffer is returned in case its resources
    // are still in use.
    pub fn resize(&mut self, size: Size2D<i32>) -> Result<DrawBuffer, &'static str> {
        let old_buffer = match self.draw_buffer.take() {
            None => return Err("No draw buffer available"),
            Some(old_buffer) => old_buffer,
        };

        self.draw_buffer = Some(DrawBuffer::new(self, size, old_buffer.color_attachment_type())?);
        Ok(old_buffer)
    }

    pub fn get_extensions(&self) -> Vec<String> {
        self.extensions.clone()
    }

    fn clear_framebuffer(
        &self,
        clear_color: Option<(f32, f32, f32, f32)>,
        mask: Option<u32>,
    ) {
        let cc = clear_color.unwrap_or((0., 0., 0., 0.));
        self.gl().clear_color(cc.0, cc.1, cc.2, if !self.attributes.alpha { 1.0 } else { cc.3 });
        self.gl().clear(mask.unwrap_or(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT));
    }

    pub fn create_compatible_draw_buffer(&self,
                                         size: &Size2D<i32>,
                                         color_attachment_type: ColorAttachmentType)
                                         -> Result<DrawBuffer, &'static str> {
        let draw_buffer = self.draw_buffer.as_ref().expect("Where's the draw buffer?");
        DrawBuffer::new(self, draw_buffer.size(), draw_buffer.color_attachment_type())
    }

    fn init_offscreen(&mut self, size: Size2D<i32>, color_attachment_type: ColorAttachmentType)
                      -> Result<(), &'static str> {
        self.draw_buffer = Some(DrawBuffer::new(self, size, color_attachment_type)?);

        debug_assert!(self.is_current());
        self.clear_framebuffer(None, None);
        self.gl().scissor(0, 0, size.width, size.height);
        self.gl().viewport(0, 0, size.width, size.height);

        Ok(())
    }

    fn query_extensions(gl_: &Rc<dyn gl::Gl>, api_version: GLVersion) -> Vec<String> {
        if api_version.major_version() >=3 {
            // glGetString(GL_EXTENSIONS) is deprecated on OpenGL >= 3.x.
            // Some GL backends such as CGL generate INVALID_ENUM error when used.
            // Use the new way to query extensions on OpenGL 3.x (glStringi)
            let mut n = [0];
            unsafe {
                gl_.get_integer_v(gl::NUM_EXTENSIONS, &mut n);
            }
            let n = n[0] as usize;
            let mut extensions = Vec::with_capacity(n);
            for index in 0..n {
                extensions.push(gl_.get_string_i(gl::EXTENSIONS, index as u32))
            }
            extensions
        } else {
            let extensions = gl_.get_string(gl::EXTENSIONS);
            extensions.split(&[',',' '][..]).map(|s| s.into()).collect()
        }
    }
}

/// Describes the OpenGL version that is requested when a context is created.
#[derive(Debug, Clone, Copy)]
pub enum GLVersion {
    /// Request a specific major version
    /// The minor version is automatically selected.
    Major(u8),

    /// Request a specific major and minor version version.
    MajorMinor(u8, u8),
}

impl GLVersion {
    // Helper method to get the major version
    pub fn major_version(&self) -> u8 {
        match *self {
            GLVersion::Major(major) => major,
            GLVersion::MajorMinor(major, _) => major,
        }
    }
}

// Dispatches functions to the thread where a NativeGLContext is bound.
// Right now it's used in the WGL implementation to dispatch functions to the thread
// where the context we share from is bound. See the WGL implementation for more details.
pub trait GLContextDispatcher {
    fn dispatch(&self, f: Box<dyn Fn() + Send>);
}
