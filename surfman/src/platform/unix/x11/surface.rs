// surfman/src/platform/unix/x11/surface.rs

//! Wrapper for GL-renderable pixmaps on X11.

use crate::context::ContextID;
use crate::gl::types::{GLenum, GLint, GLuint, GLvoid};
use crate::glx::types::Display as GlxDisplay;
use crate::{gl, glx};
use crate::{Error, SurfaceID, WindowingApiError};
use super::context::{Context, GLX_FUNCTIONS, GL_FUNCTIONS};
use super::device::{Device, Quirks};
use super::error;

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_int, c_uint, c_void};
use std::ptr;
use std::thread;
use x11::glx::{GLX_VISUAL_ID, GLXPixmap};
use x11::xlib::{self, Display, Pixmap, VisualID, Window, XCreatePixmap, XDefaultScreen};
use x11::xlib::{XDefaultScreenOfDisplay, XFree, XGetGeometry, XGetVisualInfo};
use x11::xlib::{XRootWindowOfScreen, XVisualInfo};

#[cfg(feature = "sm-winit")]
use winit::Window as WinitWindow;
#[cfg(feature = "sm-winit")]
use winit::os::unix::WindowExt;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

pub struct Surface {
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) drawables: SurfaceDrawables,
    destroyed: bool,
}

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
        pixels: Option<Vec<u8>>,
    },
    Window {
        window: Window,
    },
}

pub enum SurfaceType {
    Generic { size: Size2D<i32> },
    Widget { native_widget: NativeWidget },
}

pub struct NativeWidget {
    pub(crate) window: Window,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum SurfaceKind {
    Pixmap,
    Window,
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, surface_type: &SurfaceType)
                          -> Result<Surface, Error> {
        match *surface_type {
            SurfaceType::Generic { ref size } => self.create_generic_surface(context, size),
            SurfaceType::Widget { ref native_widget } => {
                self.create_widget_surface(context, native_widget)
            }
        }
    }

    fn create_generic_surface(&mut self, context: &Context, size: &Size2D<i32>)
                              -> Result<Surface, Error> {
        let (display, glx_display) = (self.native_display.display(), self.glx_display());

        let context_descriptor = self.context_descriptor(context);
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(&context_descriptor,
                                                                     SurfaceKind::Pixmap);

        GLX_FUNCTIONS.with(|glx| {
            unsafe {
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

                // Create an X11 pixmap.
                let pixmap = XCreatePixmap(display,
                                           XRootWindowOfScreen(XDefaultScreenOfDisplay(display)),
                                           size.width as c_uint,
                                           size.height as c_uint,
                                           depth);
                if pixmap == 0 {
                    return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
                }

                // Create a GLX pixmap.
                let glx_pixmap = glx.CreatePixmap(glx_display, glx_fb_config, pixmap, ptr::null());
                if glx_pixmap == 0 {
                    return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
                }

                Ok(Surface {
                    drawables: SurfaceDrawables::Pixmap { glx_pixmap, pixmap, pixels: None },
                    size: *size,
                    context_id: context.id,
                    destroyed: false,
                })
            }
        })
    }

    fn create_widget_surface(&mut self, context: &Context, native_widget: &NativeWidget)
                             -> Result<Surface, Error> {
        let display = self.native_display.display();
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
                                  -> Result<SurfaceTexture, Error> {

        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                let mut gl_texture = 0;
                unsafe {
                    let (glx_pixmap, pixels) = match surface.drawables {
                        SurfaceDrawables::Window { .. } => {
                            drop(self.destroy_surface(context, surface));
                            return Err(Error::WidgetAttached);
                        }
                        SurfaceDrawables::Pixmap { glx_pixmap, ref pixels, .. } => {
                            (glx_pixmap, pixels)
                        }
                    };

                    drop(self.make_context_current(context));

                    // Create a texture.
                    gl.GenTextures(1, &mut gl_texture);
                    debug_assert_ne!(gl_texture, 0);
                    gl.BindTexture(gl::TEXTURE_2D, gl_texture);

                    if !self.quirks.contains(Quirks::BROKEN_GLX_TEXTURE_FROM_PIXMAP) {
                        // Bind the surface's GLX pixmap to the texture.
                        let attributes = [
                            glx::TEXTURE_FORMAT_EXT as c_int,
                                glx::TEXTURE_FORMAT_RGBA_EXT as c_int,
                            glx::TEXTURE_TARGET_EXT as c_int,
                                glx::TEXTURE_2D_EXT as c_int,
                            0,
                        ];
                        let display = self.native_display.display() as *mut GlxDisplay;
                        glx.BindTexImageEXT(display,
                                            glx_pixmap,
                                            glx::FRONT_EXT as c_int,
                                            attributes.as_ptr());
                    } else {
                        // `GLX_texture_from_pixmap` is broken. Bummer. Copy data that was read
                        // back from the CPU.
                        match *pixels {
                            Some(ref pixels) => {
                                gl.TexImage2D(gl::TEXTURE_2D,
                                              0,
                                              gl::RGBA8 as GLint,
                                              surface.size.width,
                                              surface.size.height,
                                              0,
                                              gl::RGBA,
                                              gl::UNSIGNED_BYTE,
                                              (*pixels).as_ptr() as *const GLvoid);
                            }
                            None => {
                                gl.TexImage2D(gl::TEXTURE_2D,
                                              0,
                                              gl::RGBA8 as GLint,
                                              surface.size.width,
                                              surface.size.height,
                                              0,
                                              gl::RGBA,
                                              gl::UNSIGNED_BYTE,
                                              ptr::null());
                            }
                        }
                    }

                    // Initialize the texture, for convenience.
                    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                    gl.TexParameteri(gl::TEXTURE_2D,
                                     gl::TEXTURE_WRAP_S,
                                     gl::CLAMP_TO_EDGE as GLint);
                    gl.TexParameteri(gl::TEXTURE_2D,
                                     gl::TEXTURE_WRAP_T,
                                     gl::CLAMP_TO_EDGE as GLint);

                    gl.BindTexture(gl::TEXTURE_2D, 0);
                    debug_assert_eq!(gl.GetError(), gl::NO_ERROR);
                }

                Ok(SurfaceTexture { surface, gl_texture, phantom: PhantomData })
            })
        })
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // Avoid a panic and just leak the surface.
            surface.destroyed = true;
            return Err(Error::IncompatibleSurface)
        }

        self.make_context_not_current(context)?;

        match surface.drawables {
            SurfaceDrawables::Pixmap { ref mut glx_pixmap, pixmap: _, pixels: _ } => {
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

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        let glx_pixmap = match surface_texture.surface.drawables {
            SurfaceDrawables::Pixmap { glx_pixmap, .. } => glx_pixmap,
            SurfaceDrawables::Window { .. } => unreachable!(),
        };

        GLX_FUNCTIONS.with(|glx| {
            GL_FUNCTIONS.with(|gl| {
                unsafe {
                    gl.BindTexture(gl::TEXTURE_2D, surface_texture.gl_texture);

                    if !self.quirks.contains(Quirks::BROKEN_GLX_TEXTURE_FROM_PIXMAP) {
                        let display = self.native_display.display() as *mut GlxDisplay;
                        glx.ReleaseTexImageEXT(display, glx_pixmap, glx::FRONT_EXT as c_int);
                    }

                    gl.BindTexture(gl::TEXTURE_2D, 0);
                    gl.DeleteTextures(1, &mut surface_texture.gl_texture);
                    surface_texture.gl_texture = 0;
                }

                Ok(surface_texture.surface)
            })
        })
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    #[inline]
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        self.present_surface_without_context(surface)
    }

    pub(crate) fn present_surface_without_context(&self, surface: &mut Surface)
                                                  -> Result<(), Error> {
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
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    pub fn id(&self) -> SurfaceID {
        match self.drawables {
            SurfaceDrawables::Pixmap { glx_pixmap, .. } => SurfaceID(glx_pixmap as usize),
            SurfaceDrawables::Window { window } => SurfaceID(window as usize),
        }
    }

    #[inline]
    pub fn context_id(&self) -> ContextID {
        self.context_id
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }
}

impl NativeWidget {
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn from_winit_window(window: &WinitWindow) -> NativeWidget {
        unsafe {
            NativeWidget { window: window.get_xlib_window().expect("Where's the X11 window?") }
        }
    }
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
