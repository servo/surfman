// surfman/surfman/src/platform/unix/x11/context.rs
//
//! Wrapper for GLX contexts.

use crate::context::{self, CREATE_CONTEXT_MUTEX, ContextID};
use crate::gl::Gl;
use crate::gl::types::GLubyte;
use crate::glx::types::{Display as GlxDisplay, GLXContext, GLXDrawable, GLXFBConfig, GLXPixmap};
use crate::glx::{self, Glx};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion};
use crate::{SurfaceInfo, WindowingApiError};
use super::device::Device;
use super::error;
use super::ffi::GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB;
use super::ffi::GLX_CONTEXT_CORE_PROFILE_BIT_ARB;
use super::ffi::GLX_CONTEXT_PROFILE_MASK_ARB;
use super::surface::{self, Surface, SurfaceDrawables};

use euclid::default::Size2D;
use libc::{RTLD_LAZY, dlopen, dlsym};
use std::cell::Cell;
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
use x11::xlib::{self, Display, Pixmap, XDefaultScreen, XErrorEvent, XFree, XID, XSetErrorHandler};

const DUMMY_PIXMAP_SIZE: i32 = 16;

thread_local! {
    static LAST_X_ERROR_CODE: Cell<u8> = Cell::new(0);
}

thread_local! {
    #[doc(hidden)]
    pub static GL_FUNCTIONS: Gl = Gl::load_with(get_proc_address);
}

thread_local! {
    #[doc(hidden)]
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

/// Represents an OpenGL rendering context.
/// 
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
/// 
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
/// 
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
/// 
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub struct Context {
    pub(crate) glx_context: GLXContext,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<Surface, GLXDrawable>,
    gl_version: GLVersion,
    compatibility_profile: bool,
    dummy_glx_pixmap: GLXPixmap,
    #[allow(dead_code)]
    dummy_pixmap: Pixmap,
    status: ContextStatus,
}

/// Wraps a native GLX context and associated drawable.
pub struct NativeContext {
    /// The associated GLX context.
    pub glx_context: GLXContext,
    /// The drawable (pixmap or window) bound to the context.
    pub glx_drawable: GLXDrawable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ContextStatus {
    Owned,
    Referenced,
    Destroyed,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if self.status != ContextStatus::Destroyed && !thread::panicking() {
            //panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

/// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
/// 
/// These are local to a device.
#[derive(Clone)]
pub struct ContextDescriptor {
    pixmap_glx_fb_config_id: XID,
    gl_version: GLVersion,
    compatibility_profile: bool,
}

impl Device {
    /// Creates a context descriptor with the given attributes.
    /// 
    /// Context descriptors are local to this device.
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let display_guard = self.connection.lock_display();

        // Set environment variables as appropriate.
        self.adapter.set_environment_variables();

        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        let compatibility_profile = flags.contains(ContextAttributeFlags::COMPATIBILITY_PROFILE);

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
            glx::BIND_TO_TEXTURE_TARGETS_EXT as c_int,  glx::TEXTURE_RECTANGLE_BIT_EXT as c_int,
            // FIXME(pcwalton): We shouldn't have to double-buffer pbuffers in theory. However,
            // there seem to be some Mesa synchronization issues if we don't.
            GLX_DOUBLEBUFFER,                           xlib::True,
            0,
        ];

        unsafe {
            let pixmap_glx_fb_config_id = choose_fb_config_id(display_guard.display(),
                                                              display_guard.glx_display(),
                                                              pixmap_config_attributes.as_ptr())?;

            Ok(ContextDescriptor {
                pixmap_glx_fb_config_id,
                gl_version: attributes.version,
                compatibility_profile,
            })
        }
    }

    /// Creates a new OpenGL context.
    /// 
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(&mut self, descriptor: &ContextDescriptor) -> Result<Context, Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(descriptor);

        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let glx_context_profile_mask = if descriptor.compatibility_profile { 
                    GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB
                } else {
                    GLX_CONTEXT_CORE_PROFILE_BIT_ARB
                };

                // TODO(pcwalton): Fall back to `glXCreateNewContext()` if the
                // `GLX_ARB_create_context` extension isn't available.
                let attributes = [
                    glx::CONTEXT_MAJOR_VERSION_ARB as c_int, descriptor.gl_version.major as c_int,
                    glx::CONTEXT_MINOR_VERSION_ARB as c_int, descriptor.gl_version.minor as c_int,
                    GLX_CONTEXT_PROFILE_MASK_ARB,            glx_context_profile_mask,
                    0,
                ];

                let display_guard = self.connection.lock_display();

                let prev_error_handler = XSetErrorHandler(Some(xlib_error_handler));

                let glx_context = glx.CreateContextAttribsARB(display_guard.glx_display(),
                                                              glx_fb_config as *const c_void,
                                                              ptr::null(),
                                                              xlib::True,
                                                              attributes.as_ptr()) as GLXContext;
                if glx_context.is_null() {
                    let windowing_api_error = LAST_X_ERROR_CODE.with(|last_x_error_code| {
                        error::xlib_error_to_windowing_api_error(display_guard.display(),
                                                                 last_x_error_code.get())
                    });
                    return Err(Error::ContextCreationFailed(windowing_api_error));
                }

                XSetErrorHandler(prev_error_handler);

                let dummy_pixmap_size = Size2D::new(DUMMY_PIXMAP_SIZE, DUMMY_PIXMAP_SIZE);
                let (dummy_glx_pixmap, dummy_pixmap) =
                    surface::create_pixmaps(display_guard.display(),
                                            display_guard.glx_display(),
                                            glx_fb_config,
                                            &dummy_pixmap_size)?;

                let context = Context {
                    glx_context,
                    id: *next_context_id,
                    framebuffer: Framebuffer::None,
                    gl_version: descriptor.gl_version,
                    compatibility_profile: descriptor.compatibility_profile,
                    dummy_glx_pixmap,
                    dummy_pixmap,
                    status: ContextStatus::Owned,
                };
                next_context_id.0 += 1;
                Ok(context)
            }
        })
    }

    /// Wraps a native `GLXContext` in a `Context`.
    ///
    /// The native GLX context is not retained, as there is no way to do this in the GLX API.
    /// Therefore, it is the caller's responsibility to ensure that the returned `Context` object
    /// does not outlive the `GLXContext`.
    pub unsafe fn create_context_from_native_context(&self, native_context: NativeContext)
                                                     -> Result<Context, Error> {
        // Take locks.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let display_guard = self.connection.lock_display();

        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                let (gl_version, compatibility_profile);
                {
                    let _guard = CurrentContextGuard::new();

                    let ok = glx.MakeCurrent(display_guard.glx_display(),
                                             native_context.glx_drawable,
                                             native_context.glx_context);
                    if ok == xlib::False {
                        return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
                    }

                    gl_version = GLVersion::current(gl);
                    compatibility_profile =
                        context::current_context_uses_compatibility_profile(gl);
                };

                // Introspect the context to create a context descriptor.
                let glx_fb_config_id = get_fb_config_id(display_guard.glx_display(),
                                                        native_context.glx_context);
                let context_descriptor = ContextDescriptor {
                    pixmap_glx_fb_config_id: glx_fb_config_id,
                    gl_version,
                    compatibility_profile,
                };

                // Grab the framebuffer config from that descriptor.
                let glx_fb_config = self.context_descriptor_to_glx_fb_config(&context_descriptor);

                // Create dummy pixmaps as necessary.
                let dummy_pixmap_size = Size2D::new(DUMMY_PIXMAP_SIZE, DUMMY_PIXMAP_SIZE);
                let (dummy_glx_pixmap, dummy_pixmap) =
                    surface::create_pixmaps(display_guard.display(),
                                            display_guard.glx_display(),
                                            glx_fb_config,
                                            &dummy_pixmap_size)?;


                let context = Context {
                    glx_context: native_context.glx_context,
                    id: *next_context_id,
                    framebuffer: Framebuffer::External(native_context.glx_drawable),
                    gl_version,
                    compatibility_profile,
                    dummy_glx_pixmap,
                    dummy_pixmap,
                    status: ContextStatus::Referenced,
                };
                next_context_id.0 += 1;
                Ok(context)
            })
        })
    }

    /// Destroys a context.
    /// 
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.status == ContextStatus::Destroyed {
            return Ok(());
        }

        if let Framebuffer::Surface(mut surface) = mem::replace(&mut context.framebuffer,
                                                                Framebuffer::None) {
            self.destroy_surface(context, &mut surface)?;
        }

        unsafe {
            let display_guard = self.connection.lock_display();
            GLX_FUNCTIONS.with(|glx| {
                glx.DestroyPixmap(display_guard.glx_display(), context.dummy_glx_pixmap);
                context.dummy_glx_pixmap = 0;

                glx.MakeCurrent(display_guard.glx_display(), 0, ptr::null_mut());

                if context.status == ContextStatus::Owned {
                    glx.DestroyContext(display_guard.glx_display(), context.glx_context);
                }

                context.glx_context = ptr::null_mut();
            });
        }

        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let display_guard = self.connection.lock_display();
            let glx_context = context.glx_context;
            let glx_fb_config_id = get_fb_config_id(display_guard.glx_display(), glx_context);
            ContextDescriptor {
                pixmap_glx_fb_config_id: glx_fb_config_id,
                gl_version: context.gl_version,
                compatibility_profile: context.compatibility_profile,
            }
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    /// 
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        let display_guard = self.connection.lock_display();
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let glx_context = context.glx_context;
                let glx_drawable = self.context_glx_drawable(context);

                let ok = glx.MakeCurrent(display_guard.glx_display(), glx_drawable, glx_context);
                if ok == xlib::False {
                    return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
                }

                Ok(())
            }
        })
    }

    fn context_glx_drawable(&self, context: &Context) -> GLXDrawable {
        match context.framebuffer {
            Framebuffer::Surface(ref surface) => {
                match surface.drawables {
                    SurfaceDrawables::Pixmap { glx_pixmap, .. } => glx_pixmap,
                    SurfaceDrawables::Window { window } => window,
                }
            }
            Framebuffer::None => context.dummy_glx_pixmap,
            Framebuffer::External(glx_drawable) => glx_drawable,
        }
    }

    /// Removes the current OpenGL context from this thread.
    /// 
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        let display_guard = self.connection.lock_display();
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let ok = glx.MakeCurrent(display_guard.glx_display(), 0, ptr::null_mut());
                if ok == xlib::False {
                    return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
                }
                Ok(())
            }
        })
    }

    /// Fetches the address of an OpenGL function associated with this context.
    /// 
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    /// 
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        get_proc_address(symbol_name)
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        let display_guard = self.connection.lock_display();
        let glx_display = display_guard.glx_display();
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(context_descriptor);

        unsafe {
            let alpha_size = get_config_attr(glx_display, glx_fb_config, GLX_ALPHA_SIZE);
            let depth_size = get_config_attr(glx_display, glx_fb_config, GLX_DEPTH_SIZE);
            let stencil_size = get_config_attr(glx_display, glx_fb_config, GLX_STENCIL_SIZE);

            // Convert to `surfman` context attribute flags.
            let mut attribute_flags = ContextAttributeFlags::empty();
            attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
            attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
            attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

            if context_descriptor.compatibility_profile {
                attribute_flags.insert(ContextAttributeFlags::COMPATIBILITY_PROFILE);
            }

            // Create appropriate context attributes.
            ContextAttributes { flags: attribute_flags, version: context_descriptor.gl_version }
        }
    }

    pub(crate) fn context_descriptor_to_glx_fb_config(&self,
                                                      context_descriptor: &ContextDescriptor)
                                                      -> GLXFBConfig {
        let display_guard = self.connection.lock_display();
        let glx_fb_config_id = context_descriptor.pixmap_glx_fb_config_id;
        unsafe {
            get_fb_config_from_id(display_guard.display(),
                                  display_guard.glx_display(),
                                  glx_fb_config_id)
        }
    }

    /// Attaches a surface to a context for rendering.
    /// 
    /// This function takes ownership of the surface. The surface must have been created with this
    /// context, or an `IncompatibleSurface` error is returned.
    /// 
    /// If this function is called with a surface already bound, a `SurfaceAlreadyBound` error is
    /// returned. To avoid this error, first unbind the existing surface with
    /// `unbind_surface_from_context`.
    /// 
    /// If an error is returned, the surface is returned alongside it.
    pub fn bind_surface_to_context(&self, context: &mut Context, surface: Surface)
                                   -> Result<(), (Error, Surface)> {
        if context.id != surface.context_id {
            return Err((Error::IncompatibleSurface, surface));
        }

        match context.framebuffer {
            Framebuffer::None => context.framebuffer = Framebuffer::Surface(surface),
            Framebuffer::External(_) => return Err((Error::ExternalRenderTarget, surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, surface)),
        }

        // If we're current, call `make_context_current()` again to switch to the new framebuffer.
        if self.context_is_current(context) {
            drop(self.make_context_current(context));
        }

        Ok(())
    }

    /// Removes and returns any attached surface from this context.
    /// 
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(&self, context: &mut Context)
                                       -> Result<Option<Surface>, Error> {
        drop(self.flush_context_surface(context));

        let surface = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => surface,
            Framebuffer::None => return Ok(None),
            Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
        };

        // If we're current, we stay current, but with the dummy GLX pixmap attached.
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                let display_guard = self.connection.lock_display();
                if self.context_is_current(context) {
                    glx.MakeCurrent(display_guard.glx_display(),
                                    context.dummy_glx_pixmap,
                                    context.glx_context);
                }
            }
        });

        Ok(Some(surface))
    }

    fn context_is_current(&self, context: &Context) -> bool {
        unsafe {
            GLX_FUNCTIONS.with(|glx| glx.GetCurrentContext() == context.glx_context)
        }
    }

    fn flush_context_surface(&self, context: &mut Context) -> Result<(), Error> {
        // FIXME(pcwalton): Unbind afterward if necessary.
        let display_guard = self.connection.lock_display();
        self.make_context_current(context)?;

        let surface = match context.framebuffer {
            Framebuffer::Surface(ref mut surface) => surface,
            Framebuffer::None | Framebuffer::External(_) => return Ok(()),
        };

        match surface.drawables {
            SurfaceDrawables::Pixmap { glx_pixmap, .. } => {
                unsafe {
                    GL_FUNCTIONS.with(|gl| {
                        GLX_FUNCTIONS.with(|glx| {
                            gl.Flush();
                            glx.SwapBuffers(display_guard.glx_display(), glx_pixmap);
                        })
                    })
                }
            }
            SurfaceDrawables::Window { .. } => {}
        }

        Ok(())
    }

    /// Returns a unique ID representing a context.
    /// 
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }

    /// Returns various information about the surface attached to a context.
    /// 
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External(_) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }

    /// Given a context, returns its underlying GLX context object and associated drawable.
    #[inline]
    pub fn native_context(&self, context: &Context) -> NativeContext {
        NativeContext {
            glx_context: context.glx_context,
            glx_drawable: self.context_glx_drawable(context),
        }
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

unsafe fn get_fb_config_id(glx_display: *mut GlxDisplay, glx_context: GLXContext) -> XID {
    GLX_FUNCTIONS.with(|glx| {
        let mut glx_fb_config_id: i32 = 0;
        let err = glx.QueryContext(glx_display,
                                   glx_context,
                                   GLX_FBCONFIG_ID,
                                   &mut glx_fb_config_id);
        assert_eq!(err, xlib::Success as c_int);
        glx_fb_config_id as XID
    })
}

unsafe fn get_fb_config_from_id(display: *mut Display,
                                glx_display: *mut GlxDisplay,
                                glx_fb_config_id: XID)
                                -> GLXFBConfig {
    GLX_FUNCTIONS.with(|glx| {
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
    })
}

unsafe extern "C" fn xlib_error_handler(_: *mut Display, event: *mut XErrorEvent) -> c_int {
    LAST_X_ERROR_CODE.with(|error_code| error_code.set((*event).error_code));
    0
}

struct CurrentContextGuard {
    display: *mut GlxDisplay,
    glx_drawable: GLXDrawable,
    glx_context: GLXContext,
}

impl CurrentContextGuard {
    fn new() -> CurrentContextGuard {
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                CurrentContextGuard {
                    display: glx.GetCurrentDisplay(),
                    glx_context: glx.GetCurrentContext(),
                    glx_drawable: glx.GetCurrentDrawable(),
                }
            }
        })
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        GLX_FUNCTIONS.with(|glx| {
            unsafe {
                glx.MakeCurrent(self.display, self.glx_drawable, self.glx_context);
            }
        })
    }
}

