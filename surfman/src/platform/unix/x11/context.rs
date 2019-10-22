//! Wrapper for GLX contexts.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::gl::types::{GLubyte, GLuint};
use crate::gl::{self, Gl};
use crate::glx::types::{Display as GlxDisplay, GLXContext, GLXFBConfig};
use crate::glx::{self, Glx};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion, SurfaceAccess, SurfaceID};
use crate::{SurfaceType, WindowingApiError};
use super::device::{Device, Quirks, UnsafeDisplayRef};
use super::surface::{NativeWidget, Surface, SurfaceDrawables, SurfaceKind};

use euclid::default::Size2D;
use libc::{RTLD_DEFAULT, RTLD_LAZY, dlopen, dlsym};
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::slice;
use std::thread;
use x11::glx::{GLX_ALPHA_SIZE, GLX_BLUE_SIZE, GLX_DEPTH_SIZE, GLX_DOUBLEBUFFER, GLX_DRAWABLE_TYPE};
use x11::glx::{GLX_FBCONFIG_ID, GLX_GREEN_SIZE, GLX_PIXMAP_BIT, GLX_RED_SIZE, GLX_RENDER_TYPE};
use x11::glx::{GLX_RGBA_BIT, GLX_STENCIL_SIZE, GLX_STEREO, GLX_TRUE_COLOR, GLX_WINDOW_BIT};
use x11::glx::{GLX_X_RENDERABLE, GLX_X_VISUAL_TYPE};
use x11::xlib::{self, Display, XDefaultScreen, XFree, XID};

thread_local! {
    pub static GL_FUNCTIONS: Gl = Gl::load_with(get_proc_address);
}

thread_local! {
    pub static GLX_FUNCTIONS: Glx = Glx::load_with(get_proc_address);
}

lazy_static! {
    static ref GLX_GET_PROC_ADDRESS: unsafe extern "C" fn(*const GLubyte) -> *mut c_void = {
        unsafe {
            let library_name = &b"libGL.so\0"[0] as *const u8 as *const i8;
            let library = dlopen(library_name, RTLD_LAZY);
            assert!(!library.is_null());

            let symbol = &b"glXGetProcAddress\0"[0] as *const u8 as *const i8;
            let function = dlsym(library, symbol);
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
    pixmap_glx_fb_config_id: XID,
    window_glx_fb_config_id: XID,
    gl_version: GLVersion,
}

impl Device {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let (display, glx_display) = (self.native_display.display(), self.glx_display());

        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        let pixmap_config_attributes = [
            GLX_RED_SIZE,                               8,
            GLX_GREEN_SIZE,                             8,
            GLX_BLUE_SIZE,                              8,
            GLX_ALPHA_SIZE,                             alpha_size,
            GLX_DEPTH_SIZE,                             depth_size,
            GLX_STENCIL_SIZE,                           stencil_size,
            GLX_DRAWABLE_TYPE,                          GLX_PIXMAP_BIT | GLX_WINDOW_BIT,
            GLX_X_RENDERABLE,                           xlib::True,
            GLX_X_VISUAL_TYPE,                          GLX_TRUE_COLOR,
            GLX_RENDER_TYPE,                            GLX_RGBA_BIT,
            GLX_STEREO,                                 xlib::False,
            glx::BIND_TO_TEXTURE_RGBA_EXT as c_int,     xlib::True,
            glx::BIND_TO_TEXTURE_TARGETS_EXT as c_int,  glx::TEXTURE_2D_BIT_EXT as c_int,
            GLX_DOUBLEBUFFER,                           xlib::False,
            0,
        ];

        let window_config_attributes = [
            GLX_RED_SIZE,                               8,
            GLX_GREEN_SIZE,                             8,
            GLX_BLUE_SIZE,                              8,
            GLX_ALPHA_SIZE,                             alpha_size,
            GLX_DEPTH_SIZE,                             depth_size,
            GLX_STENCIL_SIZE,                           stencil_size,
            GLX_DRAWABLE_TYPE,                          GLX_WINDOW_BIT,
            GLX_X_RENDERABLE,                           xlib::True,
            GLX_X_VISUAL_TYPE,                          GLX_TRUE_COLOR,
            GLX_STEREO,                                 xlib::False,
            GLX_DOUBLEBUFFER,                           xlib::True,
            0,
        ];

        unsafe {
            let pixmap_glx_fb_config_id = choose_fb_config_id(display,
                                                              glx_display,
                                                              pixmap_config_attributes.as_ptr())?;
            let window_glx_fb_config_id = choose_fb_config_id(display,
                                                              glx_display,
                                                              window_config_attributes.as_ptr())?;

            Ok(ContextDescriptor {
                pixmap_glx_fb_config_id,
                window_glx_fb_config_id,
                gl_version: attributes.version,
            })
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

        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                // Get the current display, and wrap it.
                let glx_display = glx.GetCurrentDisplay();
                if glx_display.is_null() {
                    return Err(Error::NoCurrentContext);
                }
                let display = glx_display as *mut Display;

                let device = Device {
                    native_display: Box::new(UnsafeDisplayRef { display }),
                    quirks: Quirks::detect(),
                };

                // Get the current context.
                let glx_context = glx.GetCurrentContext();
                assert!(!glx_context.is_null());

                // Fetch the current GL version.
                let (mut major_gl_version, mut minor_gl_version) = (0, 0);
                gl.GetIntegerv(gl::MAJOR_VERSION, &mut major_gl_version);
                gl.GetIntegerv(gl::MINOR_VERSION, &mut minor_gl_version);

                // Wrap the context.
                let context = Context {
                    native_context: Box::new(UnsafeGLXContextRef { glx_context }),
                    id: *next_context_id,
                    gl_version: GLVersion::new(major_gl_version as u8, minor_gl_version as u8),
                    framebuffer: Framebuffer::External,
                };
                next_context_id.0 += 1;

                Ok((device, context))
            })
        })
    }

    pub fn create_context(&mut self,
                          descriptor: &ContextDescriptor,
                          surface_access: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Context, Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let surface_kind = match *surface_type {
            SurfaceType::Generic { .. } => SurfaceKind::Pixmap,
            SurfaceType::Widget  { .. } => SurfaceKind::Window,
        };
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(descriptor, surface_kind);

        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                // TODO(pcwalton): Fall back to `glXCreateNewContext()` if the
                // `GLX_ARB_create_context` extension isn't available.
                let attributes = [
                    glx::CONTEXT_MAJOR_VERSION_ARB as c_int, descriptor.gl_version.major as c_int,
                    glx::CONTEXT_MINOR_VERSION_ARB as c_int, descriptor.gl_version.minor as c_int,
                    0,
                ];

                let glx_display = self.glx_display();
                let glx_context = glx.CreateContextAttribsARB(glx_display,
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

                let initial_surface = self.create_surface(&context,
                                                          surface_access,
                                                          &surface_type)?;
                self.attach_surface(&mut context, initial_surface);
                self.make_context_current(&context)?;

                Ok(context)
            }
        })
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
        let glx_display = self.glx_display();
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let mut glx_fb_config_id: i32 = 0;
                let err = glx.QueryContext(glx_display,
                                           context.native_context.glx_context(),
                                           GLX_FBCONFIG_ID,
                                           &mut glx_fb_config_id);
                assert_eq!(err, xlib::Success as c_int);

                // FIXME(pcwalton): This is wrong. Instead we should do a dance to find the other
                // GLX FB config ID, depending on the one we have.
                ContextDescriptor {
                    window_glx_fb_config_id: glx_fb_config_id as XID,
                    pixmap_glx_fb_config_id: glx_fb_config_id as XID,
                    gl_version: context.gl_version,
                }
            }
        })
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        let glx_display = self.glx_display();
        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                unsafe {
                    let (glx_drawable, size);
                    match context.framebuffer {
                        Framebuffer::Surface(ref surface) => {
                            match surface.drawables {
                                SurfaceDrawables::Pixmap { glx_pixmap, .. } => {
                                    glx_drawable = glx_pixmap;
                                }
                                SurfaceDrawables::Window { window } => glx_drawable = window,
                            };
                            size = surface.size;
                        }
                        Framebuffer::None | Framebuffer::External => {
                            return Err(Error::ExternalRenderTarget)
                        }
                    }

                    let ok = glx.MakeCurrent(glx_display,
                                             glx_drawable,
                                             context.native_context.glx_context());
                    if ok == xlib::False {
                        return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
                    }

                    Ok(())
                }
            })
        })
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        let glx_display = self.glx_display();
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let ok = glx.MakeCurrent(glx_display, 0, ptr::null_mut());
                if ok == xlib::False {
                    return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
                }
                Ok(())
            }
        })
    }

    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        get_proc_address(symbol_name)
    }

    fn context_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface),
        }
    }

    fn context_surface_mut<'c>(&self, context: &'c mut Context) -> Result<&'c mut Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref mut surface) => Ok(surface),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        drop(self.flush_context_surface(context));
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

    #[inline]
    pub fn context_surface_size(&self, context: &Context) -> Result<Size2D<i32>, Error> {
        self.context_surface(context).map(|surface| surface.size())
    }

    #[inline]
    pub fn context_surface_id(&self, context: &Context) -> Result<SurfaceID, Error> {
        self.context_surface(context).map(|surface| surface.id())
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        let glx_display = self.glx_display();
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(context_descriptor,
                                                                     SurfaceKind::Pixmap);

        unsafe {
            let alpha_size = get_config_attr(glx_display, glx_fb_config, GLX_ALPHA_SIZE);
            let depth_size = get_config_attr(glx_display, glx_fb_config, GLX_DEPTH_SIZE);
            let stencil_size = get_config_attr(glx_display, glx_fb_config, GLX_STENCIL_SIZE);

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
                                                      context_descriptor: &ContextDescriptor,
                                                      kind: SurfaceKind)
                                                      -> GLXFBConfig {
        let (display, glx_display) = (self.native_display.display(), self.glx_display());
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let glx_fb_config_id = match kind {
                    SurfaceKind::Pixmap => context_descriptor.pixmap_glx_fb_config_id,
                    SurfaceKind::Window => context_descriptor.window_glx_fb_config_id,
                };

                let mut glx_fb_config_count = 0;
                let glx_fb_configs_ptr = glx.GetFBConfigs(glx_display,
                                                          XDefaultScreen(display),
                                                          &mut glx_fb_config_count);
                let glx_fb_configs = slice::from_raw_parts(glx_fb_configs_ptr,
                                                           glx_fb_config_count as usize);
                let glx_fb_config = *glx_fb_configs.iter().filter(|&glx_fb_config| {
                    get_config_attr(glx_display, *glx_fb_config, GLX_FBCONFIG_ID) as XID ==
                        glx_fb_config_id
                }).next().expect("Where's the GLX FB config?");

                XFree(glx_fb_configs_ptr as *mut c_void);
                glx_fb_config
            }
        })
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

        let pixels_slot = match surface.drawables {
            SurfaceDrawables::Pixmap { ref mut pixels, .. } => pixels,
            SurfaceDrawables::Window { .. } => return Ok(()),
        };

        let size = surface.size;
        let length = size.width as usize * size.height as usize * 4;

        let mut pixels = match mem::replace(pixels_slot, None) {
            None => vec![0; length],
            Some(mut pixels) => {
                if pixels.len() != length {
                    pixels.resize(length, 0);
                }
                pixels
            }
        };

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.ReadPixels(0,
                              0,
                              size.width,
                              size.height,
                              gl::RGBA,
                              gl::UNSIGNED_BYTE,
                              pixels.as_mut_ptr() as *mut c_void);
            }
        });

        *pixels_slot = Some(pixels);
        Ok(())
    }

    #[inline]
    pub fn present_context_surface(&self, context: &mut Context) -> Result<(), Error> {
        self.context_surface_mut(context).and_then(|surface| {
            self.present_surface_without_context(surface)
        })
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

        let glx_display = device.glx_display();
        GLX_FUNCTIONS.with(|glx| {
            glx.MakeCurrent(glx_display, 0, ptr::null_mut());
            glx.DestroyContext(glx_display, self.glx_context);
            self.glx_context = ptr::null_mut();
        });
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

pub(crate) unsafe fn get_config_attr(display: *mut GlxDisplay,
                                     glx_fb_config: GLXFBConfig,
                                     attr: c_int)
                                     -> c_int {
    GLX_FUNCTIONS.with(|glx| {
        let mut value = 0;
        let err = glx.GetFBConfigAttrib(display, glx_fb_config, attr, &mut value);
        assert_eq!(err, xlib::Success as c_int);
        value
    })
}

unsafe fn choose_fb_config_id(display: *mut Display,
                              glx_display: *mut GlxDisplay,
                              config_attributes: *const c_int)
                              -> Result<XID, Error> {
    GLX_FUNCTIONS.with(|glx| {
        let mut glx_fb_config_count = 0;
        let glx_fb_configs = glx.ChooseFBConfig(glx_display,
                                                XDefaultScreen(display),
                                                config_attributes,
                                                &mut glx_fb_config_count);
        if glx_fb_configs.is_null() || glx_fb_config_count == 0 {
            return Err(Error::NoPixelFormatFound);
        }

        let glx_fb_config = *glx_fb_configs;
        XFree(glx_fb_configs as *mut c_void);

        Ok(get_config_attr(glx_display, glx_fb_config, GLX_FBCONFIG_ID) as XID)
    })
}

fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        (*GLX_GET_PROC_ADDRESS)(symbol_name.as_ptr() as *const u8) as *const c_void
    }
}

