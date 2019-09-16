//! Wrapper for GLX contexts.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLFlavor, GLInfo, GLVersion};
use super::adapter::Adapter;
use super::device::Device;
use super::error;
use super::surface::{Framebuffer, Surface};
use euclid::default::Size2D;
use gl;
use gl::types::GLuint;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;
use x11::glx::{GLX_ALPHA_SIZE, GLX_DEPTH_SIZE, GLX_DRAWABLE_TYPE, GLX_CONTEXT_MAJOR_VERSION_ARB};
use x11::glx::{GLX_CONTEXT_MINOR_VERSION_ARB, GLX_PIXMAP_BIT, GLX_RENDER_BIT, GL_RGBA_BIT};
use x11::glx::{GLX_STENCIL_SIZE, glXChooseFBConfig, glXGetCurrentContext, glXGetCurrentDisplay};
use x11::glx::{glXGetProcAddress, glXMakeCurrent, glXQueryContext};
use x11::xlib::{self, XDefaultScreen, XFree, XID};

lazy_static! {
    static ref CREATE_CONTEXT_MUTEX: Mutex<ContextID> = Mutex::new(ContextID(0));
}

#[derive(Clone, Copy, PartialEq)]
pub struct ContextID(pub u64);

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    pub(crate) gl_info: GLInfo,
    gl_version: GLVersion,
    framebuffer: Framebuffer,
}

pub(crate) trait NativeContext {
    fn glx_context(&self) -> GLXContext;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self, device: &Device);
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.native_context.is_destroyed() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

#[derive(Clone)]
pub struct ContextDescriptor {
    glx_fb_config_id: XID,
    gl_version: GLVersion,
}

impl Device {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        let config_attributes = [
            GLX_ALPHA_SIZE,     alpha_size,
            GLX_DEPTH_SIZE,     depth_size,
            GLX_STENCIL_SIZE,   stencil_size,
            GLX_DRAWABLE_TYPE,  GLX_PIXMAP_BIT,
            GLX_X_RENDERABLE,   xlib::True,
            GLX_RENDER_TYPE,    GLX_RGBA_BIT,
            xlib::None,
        ];

        unsafe {
            let mut glx_fb_config_count = 0;
            let glx_fb_configs = glXChooseFBConfig(self.display,
                                                   XDefaultScreen(0),
                                                   &config_attributes,
                                                   &mut glx_fb_config_count);
            if glx_fb_configs.is_null() || glx_fb_config_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            let glx_fb_config = *glx_fb_configs;
            XFree(glx_fb_configs as *mut c_void);

            let mut glx_fb_config_id = 0;
            let err = glXGetFBConfigAttrib(self.display,
                                           glx_fb_config,
                                           GLX_FBCONFIG_ID,
                                           &mut glx_fb_config_id);
            if err != xlib::Success {
                return Err(Error::PixelFormatSelectionFailed(err));
            }

            Ok(ContextDescriptor { glx_fb_config, gl_version: attributes.flavor.api })
        }
    }

    /// Opens the device and context corresponding to the current GLX context.
    ///
    /// The native context is not retained, as there is no way to do this in the GLX API. It is the
    /// caller's responsibility to keep it alive for the duration of this context. Be careful when
    /// using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library; for example, by Glutin. It's legal to use this method to wrap a context rendering
    /// to any target: either a window or a pbuffer. The target is opaque to `surfman`; the library
    /// will not modify or try to detect the render target. This means that any of the methods that
    /// query or replace the surface—e.g. `replace_context_surface`—will fail if called with a
    /// context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Get the current display, and wrap it.
        let display = glXGetCurrentDisplay();
        if display.is_null() {
            return Err(Error::NoCurrentContext);
        }
        let device = Device { native_display: Box::new(UnsafeDisplayRef { display }) };

        // Get the current context.
        let glx_context = glXGetCurrentContext();
        assert!(!glx_context.is_null());

        // Fetch the current GL version.
        let (mut major_gl_version, minor_gl_version) = (0, 0);
        gl::GetIntegerv(gl::MAJOR_VERSION, &mut major_gl_version);
        gl::GetIntegerv(gl::MINOR_VERSION, &mut minor_gl_version);

        // Wrap the context.
        let mut context = Context {
            native_context: Box::new(UnsafeGLXContextRef { glx_context }),
            id: *next_context_id,
            gl_info: GLInfo::new(),
            version: GLVersion::new(major_gl_version as u8, minor_gl_version as u8),
            framebuffer: Framebuffer::External,
        };

        device.load_gl_functions_if_necessary(&mut context, &mut *next_context_id);

        let context_descriptor = device.context_descriptor(&context);
        let context_attributes = device.context_descriptor_attributes(&context_descriptor);
        context.gl_info.populate(&context_attributes);

        Ok((device, context))
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor, size: &Size2D<i32>)
                          -> Result<Context, Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let glx_fb_config = self.context_descriptor_to_glx_fb_config(descriptor);
        unsafe {
            // TODO(pcwalton): Fall back to `glXCreateNewContext()` if the `GLX_ARB_create_context`
            // extension isn't available.
            let attributes = [
                GLX_CONTEXT_MAJOR_VERSION_ARB,  descriptor.version.major,
                GLX_CONTEXT_MINOR_VERSION_ARB,  descriptor.version.minor,
                xlib::None,
            ];

            let mut glx_context = glXCreateContextAttribsARB(self.native_display.display(),
                                                             glx_fb_config,
                                                             ptr::null(),
                                                             x11::True,
                                                             &attributes);
            if glx_context.is_null() {
                return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
            }

            let mut context = Context {
                native_context: Box::new(OwnedGLXContext { glx_context }),
                id: *next_context_id,
                framebuffer: Framebuffer::None,
                gl_info: GLInfo::new(),
            };

            self.load_gl_functions_if_necessary(&mut context, &mut *next_context_id);

            let context_descriptor = self.context_descriptor(&context);
            let context_attributes = self.context_descriptor_attributes(&context_descriptor);
            context.gl_info.populate(&context_attributes);

            let initial_surface = self.create_surface(&context, size)?;
            self.attach_surface(&mut context, initial_surface);
            self.make_context_current(&context)?;

            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Framebuffer::Surface(surface) = mem::replace(&mut context.framebuffer,
                                                            Framebuffer::None) {
            self.destroy_surface(context, surface)?;
        }

        unsafe {
            context.native_context.destroy();
        }

        Ok(())
    }

    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let mut glx_fb_config_id = xlib::None;
            let err = glXQueryContext(self.native_display.display(),
                                      context.native_context.glx_context(),
                                      GLX_FBCONFIG_ID,
                                      &mut glx_fb_config_id);
            assert_eq!(err, xlib::Success);
            ContextDescriptor { glx_fb_config_id, gl_version: context.version }
        }
    }

    #[inline]
    pub fn context_gl_info<'c>(&self, context: &'c Context) -> &'c GLInfo {
        &context.gl_info
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let (glx_drawable, size) = match context.framebuffer {
                Framebuffer::Surface(ref surface) => (surface.glx_drawable, surface.size),
                Framebuffer::None | Framebuffer::External => {
                    return Err(Error::ExternalRenderTarget)
                }
            };

            let ok = glXMakeCurrent(self.native_display.display(),
                                    glx_drawable,
                                    context.native_context.context());
            if ok == xlib::False {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }

            gl::Viewport(0, 0, size.width, size.height);
            Ok(())
        }
    }

    pub fn make_context_not_current(&self, _: &Context) -> Result<(), Error> {
        unsafe {
            let ok = glXMakeCurrent(self.native_display.display(), xlib::None, ptr::null());
            if ok == xlib::False {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }
            Ok(())
        }
    }

    pub fn get_proc_address(&self, _: &Context, symbol_name: &str)
                            -> Result<*const c_void, Error> {
        unsafe {
            let symbol_name: CString = CString::new(symbol_name).unwrap();
            let fun_ptr = glXGetProcAddress(symbol_name.as_ptr());
            if fun_ptr.is_null() {
                return Err(Error::GLFunctionNotFound);
            }

            return Ok(fun_ptr as *const c_void);
        }
    }

    #[inline]
    pub fn context_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        let old_surface = self.release_surface(context).expect("Where's our surface?");
        self.attach_surface(context, new_surface);
        self.make_context_current(context)?;

        Ok(old_surface)
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface.framebuffer_object),
        }
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        let display = self.native_display.display();
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(context_descriptor);

        unsafe {
            let alpha_size = get_config_attr(display, glx_fb_config, GLX_ALPHA_SIZE);
            let depth_size = get_config_attr(display, glx_fb_config, GLX_DEPTH_SIZE);
            let stencil_size = get_config_attr(display, glx_fb_config, GLX_STENCIL_SIZE);

            // Convert to `surfman` context attribute flags.
            let mut attribute_flags = ContextAttributeFlags::empty();
            attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
            attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
            attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

            // Generate an appropriate GL flavor.
            let flavor = GLFlavor { api: GLApi::GL, version: context_descriptor.gl_version };

            // Create appropriate context attributes.
            ContextAttributes { flags: attribute_flags, flavor }
        }
    }

    fn load_gl_functions_if_necessary(&self,
                                      mut context: &mut Context,
                                      next_context_id: &mut ContextID) {
        // Load the GL functions via GLX if this is the first context created.
        if *next_context_id == ContextID(0) {
            gl::load_with(|symbol| {
                self.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
            });
        }

        next_context_id.0 += 1;
    }
}

struct OwnedGLXContext {
    glx_context: GLXContext,
}

impl NativeContext for OwnedGLXContext {
    #[inline]
    fn glx_context(&self) -> GLXContext {
        debug_assert!(!self.is_destroyed());
        self.glx_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.glx_context.is_null()
    }

    unsafe fn destroy(&mut self, device: &Device) {
        assert!(!self.is_destroyed());
        let display = device.native_display.display();
        glXMakeCurrent(display, xlib::None, ptr::null());
        glXDestroyContext(display, self.glx_context);
        self.glx_context = ptr::null_mut();
    }
}

struct UnsafeGLXContextRef {
    glx_context: GLXContext,
}

impl NativeContext for UnsafeGLXContextRef {
    #[inline]
    fn glx_context(&self) -> GLXContext {
        self.glx_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.glx_context.is_null()
    }

    unsafe fn destroy(&mut self, device: &Device) {
        assert!(!self.is_destroyed());
        self.glx_context = ptr::null_mut();
    }
}

