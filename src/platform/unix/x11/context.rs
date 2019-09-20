//! Wrapper for GLX contexts.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLVersion, WindowingApiError};
use super::device::{Device, Quirks, UnsafeDisplayRef};
use super::surface::Surface;

use crate::glx::types::Display as GlxDisplay;
use crate::glx;
use euclid::default::Size2D;
use gl;
use gl::types::GLuint;
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::Mutex;
use std::thread;
use x11::glx::{GLX_ALPHA_SIZE};
use x11::glx::{GLX_DEPTH_SIZE, GLX_DOUBLEBUFFER, GLX_DRAWABLE_TYPE};
use x11::glx::{GLX_FBCONFIG_ID, GLX_PIXMAP_BIT};
use x11::glx::{GLX_RENDER_TYPE, GLX_RGBA_BIT, GLX_STENCIL_SIZE};
use x11::glx::{GLX_X_RENDERABLE, GLXContext, GLXFBConfig, glXChooseFBConfig, glXDestroyContext};
use x11::glx::{glXGetCurrentContext, glXGetCurrentDisplay, glXGetFBConfigAttrib};
use x11::glx::{glXGetProcAddress, glXMakeCurrent, glXQueryContext, glXSwapBuffers};
use x11::xlib::{self, Display, XDefaultScreen, XFree, XID};

lazy_static! {
    static ref GLX_GET_PROC_ADDRESS: extern "C" fn(*const GLubyte) -> *mut c_void = {
        unsafe {
            let symbol = &b"glXGetProcAddress\0"[0] as *const u8 as *const i8;
            let function = dlsym(RTLD_DEFAULT, symbol);
            assert!(!function.is_null());
            mem::transmute(function)
        }
    };
}

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<Surface>,
    gl_version: GLVersion,
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
        let display = self.native_display.display();

        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        let config_attributes = [
            GLX_ALPHA_SIZE,                             alpha_size,
            GLX_DEPTH_SIZE,                             depth_size,
            GLX_STENCIL_SIZE,                           stencil_size,
            GLX_DRAWABLE_TYPE,                          GLX_PIXMAP_BIT,
            GLX_X_RENDERABLE,                           xlib::True,
            GLX_RENDER_TYPE,                            GLX_RGBA_BIT,
            glx::BIND_TO_TEXTURE_RGBA_EXT as c_int,     xlib::True,
            glx::BIND_TO_TEXTURE_TARGETS_EXT as c_int,  glx::TEXTURE_2D_BIT_EXT as c_int,
            GLX_DOUBLEBUFFER,                           xlib::False,
            0,
        ];

        unsafe {
            let mut glx_fb_config_count = 0;
            let glx_fb_configs = glXChooseFBConfig(display,
                                                   XDefaultScreen(display),
                                                   config_attributes.as_ptr(),
                                                   &mut glx_fb_config_count);
            if glx_fb_configs.is_null() || glx_fb_config_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            let glx_fb_config = *glx_fb_configs;
            XFree(glx_fb_configs as *mut c_void);

            let glx_fb_config_id = get_config_attr(display, glx_fb_config, GLX_FBCONFIG_ID) as XID;
            Ok(ContextDescriptor { glx_fb_config_id, gl_version: attributes.version })
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
        let device = Device {
            native_display: Box::new(UnsafeDisplayRef { display }),
            quirks: Quirks::detect(),
        };

        // Get the current context.
        let glx_context = glXGetCurrentContext();
        assert!(!glx_context.is_null());

        // Fetch the current GL version.
        let (mut major_gl_version, mut minor_gl_version) = (0, 0);
        gl::GetIntegerv(gl::MAJOR_VERSION, &mut major_gl_version);
        gl::GetIntegerv(gl::MINOR_VERSION, &mut minor_gl_version);

        // Wrap the context.
        let mut context = Context {
            native_context: Box::new(UnsafeGLXContextRef { glx_context }),
            id: *next_context_id,
            gl_version: GLVersion::new(major_gl_version as u8, minor_gl_version as u8),
            framebuffer: Framebuffer::External,
        };
        next_context_id.0 += 1;

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
                glx::CONTEXT_MAJOR_VERSION_ARB as c_int, descriptor.gl_version.major as c_int,
                glx::CONTEXT_MINOR_VERSION_ARB as c_int, descriptor.gl_version.minor as c_int,
                0,
            ];

            let display = self.native_display.display() as *mut GlxDisplay;
            let glx_context = glx::CreateContextAttribsARB(display,
                                                           glx_fb_config as *const c_void,
                                                           ptr::null(),
                                                           xlib::True,
                                                           attributes.as_ptr()) as GLXContext;
            if glx_context.is_null() {
                return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
            }

            let mut context = Context {
                native_context: Box::new(OwnedGLXContext { glx_context }),
                id: *next_context_id,
                framebuffer: Framebuffer::None,
                gl_version: descriptor.gl_version,
            };
            next_context_id.0 += 1;

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
            context.native_context.destroy(self);
        }

        Ok(())
    }

    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let mut glx_fb_config_id: i32 = 0;
            let err = glXQueryContext(self.native_display.display(),
                                      context.native_context.glx_context(),
                                      GLX_FBCONFIG_ID,
                                      &mut glx_fb_config_id);
            assert_eq!(err, xlib::Success as c_int);
            ContextDescriptor {
                glx_fb_config_id: glx_fb_config_id as XID,
                gl_version: context.gl_version,
            }
        }
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let (glx_drawable, size) = match context.framebuffer {
                Framebuffer::Surface(ref surface) => (surface.glx_pixmap, surface.size),
                Framebuffer::None | Framebuffer::External => {
                    return Err(Error::ExternalRenderTarget)
                }
            };

            let ok = glXMakeCurrent(self.native_display.display(),
                                    glx_drawable,
                                    context.native_context.glx_context());
            if ok == xlib::False {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }

            gl::Viewport(0, 0, size.width, size.height);
            Ok(())
        }
    }

    pub fn make_context_not_current(&self, _: &Context) -> Result<(), Error> {
        unsafe {
            let ok = glXMakeCurrent(self.native_display.display(), 0, ptr::null_mut());
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
            match glXGetProcAddress(symbol_name.as_ptr() as *const u8) {
                None => Err(Error::GLFunctionNotFound),
                Some(fun_ptr) => Ok(fun_ptr as *const c_void),
            }
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
        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        self.flush_context_surface(context);
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
            Framebuffer::Surface(_) => Ok(0),
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

            // Create appropriate context attributes.
            ContextAttributes { flags: attribute_flags, version: context_descriptor.gl_version }
        }
    }

    pub(crate) fn context_descriptor_to_glx_fb_config(&self,
                                                      context_descriptor: &ContextDescriptor)
                                                      -> GLXFBConfig {
        let display = self.native_display.display();
        unsafe {
            let config_attributes = [
                GLX_FBCONFIG_ID, context_descriptor.glx_fb_config_id as c_int,
                0,
            ];

            let mut glx_fb_config_count = 0;
            let glx_fb_configs = glXChooseFBConfig(display,
                                                   XDefaultScreen(display),
                                                   config_attributes.as_ptr(),
                                                   &mut glx_fb_config_count);
            assert!(!glx_fb_configs.is_null());
            assert!(glx_fb_config_count != 0);

            let glx_fb_config = *glx_fb_configs;
            XFree(glx_fb_configs as *mut c_void);
            glx_fb_config
        }
    }

    fn attach_surface(&self, context: &mut Context, surface: Surface) {
        match context.framebuffer {
            Framebuffer::None => context.framebuffer = Framebuffer::Surface(surface),
            _ => panic!("Tried to attach a surface, but there was already a surface present!"),
        }
    }

    fn release_surface(&self, context: &mut Context) -> Option<Surface> {
        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => Some(surface),
            Framebuffer::None | Framebuffer::External => None,
        }
    }

    fn flush_context_surface(&self, context: &mut Context) -> Result<(), Error> {
        if !self.quirks.contains(Quirks::BROKEN_GLX_TEXTURE_FROM_PIXMAP) {
            return Ok(())
        }

        self.make_context_current(context)?;

        let surface = match context.framebuffer {
            Framebuffer::Surface(ref mut surface) => surface,
            Framebuffer::None | Framebuffer::External => return Ok(()),
        };

        let length = surface.size.width as usize * surface.size.height as usize * 4;
        let mut pixels = match mem::replace(&mut surface.pixels, None) {
            None => vec![0; length],
            Some(mut pixels) => {
                if pixels.len() != length {
                    pixels.resize(length, 0);
                }
                pixels
            }
        };

        unsafe {
            gl::ReadPixels(0,
                           0,
                           surface.size.width,
                           surface.size.height,
                           gl::RGBA,
                           gl::UNSIGNED_BYTE,
                           pixels.as_mut_ptr() as *mut c_void);
        }

        surface.pixels = Some(pixels);
        Ok(())
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
        glXMakeCurrent(display, 0, ptr::null_mut());
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

    unsafe fn destroy(&mut self, _: &Device) {
        assert!(!self.is_destroyed());
        self.glx_context = ptr::null_mut();
    }
}

pub(crate) unsafe fn get_config_attr(display: *mut Display,
                                     glx_fb_config: GLXFBConfig,
                                     attr: c_int)
                                     -> c_int {
    let mut value = 0;
    let err = glXGetFBConfigAttrib(display, glx_fb_config, attr, &mut value);
    assert_eq!(err, xlib::Success as c_int);
    value
}
