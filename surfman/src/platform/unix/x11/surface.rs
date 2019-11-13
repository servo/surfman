// surfman/src/platform/unix/x11/surface.rs
//
//! Wrapper for GL-renderable pixmaps on X11.

use crate::context::ContextID;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::glx::types::{Display as GlxDisplay, GLXFBConfig};
use crate::{gl, glx};
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType, WindowingApiError};
use super::context::{Context, GLX_FUNCTIONS, GL_FUNCTIONS};
use super::device::Device;
use super::error;

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_int, c_uint, c_void};
use std::thread;
use x11::glx::{GLX_VISUAL_ID, GLXPixmap};
use x11::xlib::{self, Display, Pixmap, VisualID, Window, XCreatePixmap, XDefaultScreen};
use x11::xlib::{XDefaultScreenOfDisplay, XFree, XGetGeometry, XGetVisualInfo};
use x11::xlib::{XRootWindowOfScreen, XVisualInfo};

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_RECTANGLE;

static GLX_PIXMAP_ATTRIBUTES: [c_int; 5] = [
    glx::TEXTURE_FORMAT_EXT as c_int, glx::TEXTURE_FORMAT_RGBA_EXT as c_int,
    glx::TEXTURE_TARGET_EXT as c_int, glx::TEXTURE_RECTANGLE_EXT as c_int,
    0,
];

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
/// 
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
/// 
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
/// 
/// Depending on the platform, each surface may be internally double-buffered.
/// 
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
pub struct Surface {
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) drawables: SurfaceDrawables,
    destroyed: bool,
}

/// Represents an OpenGL texture that wraps a surface.
/// 
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
/// 
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

pub(crate) enum SurfaceDrawables {
    Pixmap {
        glx_pixmap: GLXPixmap,
        #[allow(dead_code)]
        pixmap: Pixmap,
    },
    Window {
        window: Window,
    },
}

#[derive(Clone)]
pub struct NativeWidget {
    pub(crate) window: Window,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    /// 
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(&mut self,
                          context: &Context,
                          _: SurfaceAccess,
                          surface_type: SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { native_widget } => {
                self.create_widget_surface(context, native_widget)
            }
        }
    }

    fn create_generic_surface(&mut self, context: &Context, size: &Size2D<i32>)
                              -> Result<Surface, Error> {
        let display = self.connection.native_display.display();
        let glx_display = self.glx_display();

        let context_descriptor = self.context_descriptor(context);
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(&context_descriptor);

        unsafe {
            let (glx_pixmap, pixmap) = create_pixmaps(display, glx_display, glx_fb_config, size)?;
            Ok(Surface {
                drawables: SurfaceDrawables::Pixmap { glx_pixmap, pixmap },
                size: *size,
                context_id: context.id,
                destroyed: false,
            })
        }
    }

    fn create_widget_surface(&mut self, context: &Context, native_widget: NativeWidget)
                             -> Result<Surface, Error> {
        let display = self.connection.native_display.display();
        unsafe {
            let (mut root_window, mut x, mut y, mut width, mut height) = (0, 0, 0, 0, 0);
            let (mut border_width, mut depth) = (0, 0);
            XGetGeometry(display,
                         native_widget.window,
                         &mut root_window,
                         &mut x,
                         &mut y,
                         &mut width,
                         &mut height,
                         &mut border_width,
                         &mut depth);
            Ok(Surface {
                size: Size2D::new(width as i32, height as i32),
                context_id: context.id,
                drawables: SurfaceDrawables::Window { window: native_widget.window },
                destroyed: false,
            })
        }
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, (Error, Surface)> {

        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                let mut gl_texture = 0;
                unsafe {
                    let glx_pixmap = match surface.drawables {
                        SurfaceDrawables::Window { .. } => {
                            return Err((Error::WidgetAttached, surface));
                        }
                        SurfaceDrawables::Pixmap { glx_pixmap, .. } => glx_pixmap,
                    };

                    drop(self.make_context_current(context));

                    // Create a texture.
                    gl.GenTextures(1, &mut gl_texture);
                    debug_assert_ne!(gl_texture, 0);
                    gl.BindTexture(gl::TEXTURE_RECTANGLE, gl_texture);

                    // Initialize the texture, for convenience.
                    gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                     gl::TEXTURE_MAG_FILTER,
                                     gl::LINEAR as GLint);
                    gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                     gl::TEXTURE_MIN_FILTER,
                                     gl::LINEAR as GLint);
                    gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                     gl::TEXTURE_WRAP_S,
                                     gl::CLAMP_TO_EDGE as GLint);
                    gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                     gl::TEXTURE_WRAP_T,
                                     gl::CLAMP_TO_EDGE as GLint);

                    // Bind the surface's GLX pixmap to the texture.
                    let display = self.connection.native_display.display() as *mut GlxDisplay;
                    glx.BindTexImageEXT(display,
                                        glx_pixmap,
                                        glx::FRONT_EXT as c_int,
                                        GLX_PIXMAP_ATTRIBUTES.as_ptr());

                    gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);
                    debug_assert_eq!(gl.GetError(), gl::NO_ERROR);
                }

                Ok(SurfaceTexture { surface, gl_texture, phantom: PhantomData })
            })
        })
    }

    /// Destroys a surface.
    /// 
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    /// 
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(&self, context: &mut Context, surface: &mut Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface)
        }

        self.make_no_context_current()?;

        match surface.drawables {
            SurfaceDrawables::Pixmap { ref mut glx_pixmap, pixmap: _ } => {
                let glx_display = self.glx_display();
                GLX_FUNCTIONS.with(|glx| {
                    unsafe {
                        glx.DestroyPixmap(glx_display, *glx_pixmap);
                        *glx_pixmap = 0;
                    }
                });
            }
            SurfaceDrawables::Window { .. } => {}
        }

        surface.destroyed = true;
        Ok(())
    }

    /// Destroys a surface texture and returns the underlying surface.
    /// 
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    /// 
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, (Error, SurfaceTexture)> {
        let glx_pixmap = match surface_texture.surface.drawables {
            SurfaceDrawables::Pixmap { glx_pixmap, .. } => glx_pixmap,
            SurfaceDrawables::Window { .. } => unreachable!(),
        };

        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                unsafe {
                    gl.BindTexture(gl::TEXTURE_RECTANGLE, surface_texture.gl_texture);

                    // Release the GLX pixmap.
                    let display = self.connection.native_display.display() as *mut GlxDisplay;
                    glx.ReleaseTexImageEXT(display, glx_pixmap, glx::FRONT_EXT as c_int);

                    gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);
                    gl.DeleteTextures(1, &mut surface_texture.gl_texture);
                    surface_texture.gl_texture = 0;
                }

                Ok(surface_texture.surface)
            })
        })
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    /// 
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
        Err(Error::Unimplemented)
    }

    /// Displays the contents of a widget surface on screen.
    /// 
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    /// 
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        // TODO(pcwalton): Use the `XPRESENT` extension.
        unsafe {
            GLX_FUNCTIONS.with(|glx| {
                match surface.drawables {
                    SurfaceDrawables::Window { window } => {
                        glx.SwapBuffers(self.glx_display(), window);
                        Ok(())
                    }
                    SurfaceDrawables::Pixmap { .. } => Err(Error::NoWidgetAttached),
                }
            })
        }
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    /// 
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    #[inline]
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        SurfaceInfo {
            size: surface.size,
            id: surface.id(),
            context_id: surface.context_id,
            framebuffer_object: 0,
        }
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    /// 
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.gl_texture
    }
}

impl Surface {
    fn id(&self) -> SurfaceID {
        match self.drawables {
            SurfaceDrawables::Pixmap { glx_pixmap, .. } => SurfaceID(glx_pixmap as usize),
            SurfaceDrawables::Window { window } => SurfaceID(window as usize),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}

unsafe fn get_depth_of_visual_with_id(display: *mut Display, visual_id: VisualID)
                                      -> Option<c_uint> {
    let mut visual_info_template: XVisualInfo = mem::zeroed();
    visual_info_template.screen = XDefaultScreen(display);
    visual_info_template.visualid = visual_id;

    let mut matched_visual_infos_count = 0;
    let matched_visual_infos = XGetVisualInfo(display,
                                              xlib::VisualIDMask | xlib::VisualScreenMask,
                                              &mut visual_info_template,
                                              &mut matched_visual_infos_count);
    if matched_visual_infos_count == 0 || matched_visual_infos.is_null() {
        return None;
    }

    let depth = (*matched_visual_infos).depth as c_uint;
    XFree(matched_visual_infos as *mut c_void);
    Some(depth)
}

pub(crate) unsafe fn create_pixmaps(display: *mut Display,
                                    glx_display: *mut GlxDisplay,
                                    glx_fb_config: GLXFBConfig,
                                    size: &Size2D<i32>)
                                    -> Result<(GLXPixmap, Pixmap), Error> {
    GLX_FUNCTIONS.with(|glx| {
        let mut glx_visual_id = 0;
        let result = glx.GetFBConfigAttrib(glx_display,
                                           glx_fb_config,
                                           GLX_VISUAL_ID,
                                           &mut glx_visual_id);
        if result != xlib::Success as c_int {
            let windowing_api_error = error::glx_error_to_windowing_api_error(result);
            return Err(Error::SurfaceCreationFailed(windowing_api_error));
        }

        // Get the depth of the current visual.
        let depth = get_depth_of_visual_with_id(display, glx_visual_id as VisualID);
        let depth = depth.expect("GLX FB config has an invalid visual ID!");

        let pixmap = XCreatePixmap(display,
                                   XRootWindowOfScreen(XDefaultScreenOfDisplay(display)),
                                   size.width as u32,
                                   size.height as u32,
                                   depth);
        if pixmap == 0 {
            return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
        }

        // The Khronos documentation page states that `attributes` must be null. This is a filthy
        // lie. In reality, Mesa expects these attributes to be the same as those passed to
        // `glXBindTexImageEXT`. Following the documentation will result in no errors but will
        // produce a black texture.
        let glx_pixmap = glx.CreatePixmap(glx_display,
                                          glx_fb_config,
                                          pixmap,
                                          GLX_PIXMAP_ATTRIBUTES.as_ptr());
        if glx_pixmap == 0 {
            return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
        }

        Ok((glx_pixmap, pixmap))
    })
}

