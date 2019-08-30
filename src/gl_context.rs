//! Wraps a native graphics context and manages its render target.

use euclid::default::Size2D;
use gl::{self, GLuint};
use std::marker::PhantomData;
use std::mem;
use std::rc::Rc;

use crate::{Display, GLContextAttributes, GLContextCapabilities, GLFormats, GLLimits};
use crate::platform::{Context, Surface, SurfaceTexture};
use crate::render_target::RenderTarget;

/// This is a wrapper over a native headless GL context
pub struct GLContext {
    device: Device,
    context: Context,
    render_target: RenderTarget,
    flavor: GLFlavor,
    info: GLInfo,
    extensions: Vec<String>,
}

impl GLContext {
    pub fn new(display: Display,
               flavor: &GLFlavor,
               dispatcher: Option<Box<dyn GLContextDispatcher>>)
               -> Result<Self, &'static str> {
        let context = Context::new(flavor, dispatcher)?;

        let gl = unsafe {
            match flavor.api_type {
                gl::GlType::Gl => gl::GlFns::load_with(|s| Self::get_proc_address(s) as *const _),
                gl::GlType::Gles => {
                    gl::GlesFns::load_with(|s| Self::get_proc_address(s) as *const _)
                }
            }
        };

        native_context.make_current()?;

        let extensions = Self::query_extensions(&gl, api_version);
        let attributes = GLContextAttributes::any();
        let formats = GLFormats::detect(&attributes, &extensions[..], flavor);
        let limits = GLLimits::detect(&*gl);

        Ok(GLContext {
            display,
            gl,
            flavor: *flavor,
            native_context,
            render_target: RenderTarget::Unallocated,
            attributes,
            capabilities: GLContextCapabilities::detect(),
            formats,
            limits,
            extensions,
        })
    }

    #[inline]
    pub fn get_proc_address(addr: &str) -> *const () {
        Context::get_proc_address(addr)
    }

    #[inline]
    pub fn make_current(&self) -> Result<(), &'static str> {
        self.context.make_current()
    }

    #[inline]
    pub fn unbind(&self) -> Result<(), &'static str> {
        let ret = self.context.unbind();

        // OSMesa doesn't allow any API to unbind a context before [1], and just
        // bails out on null context, buffer, or whatever, so not much we can do
        // here. Thus, ignore the failure and just flush the context if we're
        // using an old OSMesa version.
        //
        // [1]: https://www.mail-archive.com/mesa-dev@lists.freedesktop.org/msg128408.html
        if self.context.is_osmesa() && ret.is_err() {
            self.gl().flush();
            return Ok(())
        }

        ret
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        self.context.is_current()
    }

    #[inline]
    pub fn gl(&self) -> &dyn gl::Gl {
        &*self.gl
    }

    #[inline]
    pub fn clone_gl(&self) -> Rc<dyn gl::Gl> {
        self.gl.clone()
    }

    #[inline]
    pub fn api_type(&self) -> gl::GlType {
        self.api_type
    }

    #[inline]
    pub fn api_version(&self) -> GLVersion {
        self.api_version
    }

    #[inline]
    pub fn attributes(&self) -> &GLContextAttributes {
        &self.attributes
    }

    #[inline]
    pub fn capabilities(&self) -> &GLContextCapabilities {
        &self.capabilities
    }

    #[inline]
    pub fn formats(&self) -> &GLFormats {
        &self.formats
    }

    #[inline]
    pub fn limits(&self) -> &GLLimits {
        &self.limits
    }

    #[inline]
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    #[inline]
    pub fn render_target(&self) -> &RenderTarget {
        &self.render_target
    }

    #[inline]
    pub fn swap_color_surface(&mut self, new_surface: Surface)
                              -> Result<Surface, &'static str> {
        match self.context.swap_default_surface(new_surface) {
            DefaultSurfaceSwapResult::Failed { new_surface: _, message } => Err(message),
            DefaultSurfaceSwapResult::Swapped { old_surface } => Ok(old_surface),
            DefaultSurfaceSwapResult::NotSupported { new_surface } => {
                self.render_target
                    .swap_color_surface(&*self.gl_, new_surface)
                    .map_err(|()| "Surface swap unsupported")
            }
        }
    }

    #[inline]
    pub fn size(&self) -> Option<Size2D<i32>> {
        self.render_target.size()
    }

    // We resize just replacing the render target, we don't perform size optimizations
    // in order to keep this generic. The old target is returned in case its resources
    // are still in use.
    pub fn resize(&mut self, size: &Size2D<i32>) -> Result<RenderTarget, &'static str> {
        let old_render_target = self.render_target.take();

        self.render_target = RenderTarget::new(self.display.clone(),
                                               &*self.gl_,
                                               &mut self.native_context,
                                               self.api_type,
                                               self.api_version,
                                               &size,
                                               &self.attributes,
                                               &self.formats)?;

        Ok(old_render_target)
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

    fn allocate_render_target(&mut self, size: &Size2D<i32>) -> Result<(), &'static str> {
        self.render_target = RenderTarget::new(self.display.clone(),
                                               &*self.gl_,
                                               &mut self.native_context,
                                               &self.flavor,
                                               &size,
                                               &self.attributes,
                                               &self.formats)?;

        debug_assert!(self.is_current());
        self.clear_framebuffer(None, None);
        self.gl().scissor(0, 0, size.width, size.height);
        self.gl().viewport(0, 0, size.width, size.height);

        Ok(())
    }

    fn query_extensions(gl: &Rc<dyn gl::Gl>, api_version: GLVersion) -> Vec<String> {
        if api_version.major_version() >=3 {
            // glGetString(GL_EXTENSIONS) is deprecated on OpenGL >= 3.x.
            // Some GL backends such as CGL generate INVALID_ENUM error when used.
            // Use the new way to query extensions on OpenGL 3.x (glStringi)
            let mut n = [0];
            unsafe {
                gl.get_integer_v(gl::NUM_EXTENSIONS, &mut n);
            }
            let n = n[0] as usize;
            let mut extensions = Vec::with_capacity(n);
            for index in 0..n {
                extensions.push(gl.get_string_i(gl::EXTENSIONS, index as u32))
            }
            extensions
        } else {
            let extensions = gl.get_string(gl::EXTENSIONS);
            extensions.split(&[',',' '][..]).map(|s| s.into()).collect()
        }
    }
}

// Dispatches functions to the thread where a NativeGLContext is bound.
// Right now it's used in the WGL implementation to dispatch functions to the thread
// where the context we share from is bound. See the WGL implementation for more details.
pub trait GLContextDispatcher {
    fn dispatch(&self, f: Box<dyn Fn() + Send>);
}
