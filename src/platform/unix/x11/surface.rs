//! Wrapper for GL-renderable pixmaps on X11.

use crate::{Error, SurfaceID, WindowingApiError};
use super::context::{Context, ContextID};
use super::device::{Device, Quirks};
use super::error;

use crate::glx::types::Display as GlxDisplay;
use crate::glx;
use euclid::default::Size2D;
use gl;
use gl::types::{GLenum, GLint, GLuint, GLvoid};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_int, c_uint, c_void};
use std::ptr;
use std::thread;
use x11::glx::{GLX_VISUAL_ID, GLXPixmap};
use x11::glx::{glXCreatePixmap, glXDestroyPixmap, glXGetFBConfigAttrib, glXMakeCurrent};
use x11::xlib::{self, Display, Pixmap, VisualID, XCreatePixmap, XDefaultScreen, XDefaultScreenOfDisplay};
use x11::xlib::{XFree, XGetVisualInfo, XRootWindowOfScreen, XVisualInfo};

pub struct Surface {
    pub(crate) glx_pixmap: GLXPixmap,
    pub(crate) pixmap: Pixmap,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) pixels: Option<Vec<u8>>,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.glx_pixmap as usize)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.glx_pixmap != 0 && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, size: &Size2D<i32>)
                          -> Result<Surface, Error> {
        let display = self.native_display.display();

        let context_descriptor = self.context_descriptor(context);
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(&context_descriptor);

        unsafe {
            let mut glx_visual_id = 0;
            let result = glXGetFBConfigAttrib(display,
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
            let glx_pixmap = glXCreatePixmap(display, glx_fb_config, pixmap, ptr::null());
            if glx_pixmap == 0 {
                return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
            }

            Ok(Surface { glx_pixmap, pixmap, size: *size, context_id: context.id, pixels: None })
        }
    }

    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        let context_descriptor = self.context_descriptor(context);
        let glx_fb_config = self.context_descriptor_to_glx_fb_config(&context_descriptor);

        unsafe {
            drop(self.make_context_current(context));

            // Create a texture.
            let mut gl_texture = 0;
            gl::GenTextures(1, &mut gl_texture);
            debug_assert_ne!(gl_texture, 0);
            gl::BindTexture(gl::TEXTURE_2D, gl_texture);

            if !self.quirks.contains(Quirks::BROKEN_GLX_TEXTURE_FROM_PIXMAP) {
                // Bind the surface's GLX pixmap to the texture.
                let attributes = [
                    glx::TEXTURE_FORMAT_EXT as c_int,   glx::TEXTURE_FORMAT_RGBA_EXT as c_int,
                    glx::TEXTURE_TARGET_EXT as c_int,   glx::TEXTURE_2D_EXT as c_int,
                    0,
                ];
                let display = self.native_display.display() as *mut GlxDisplay;
                glx::BindTexImageEXT(display,
                                     surface.glx_pixmap,
                                     glx::FRONT_EXT as c_int,
                                     attributes.as_ptr());
            } else {
                // `GLX_texture_from_pixmap` is broken. Bummer. Copy data that was read back from
                // the CPU.
                match surface.pixels {
                    Some(ref pixels) => {
                        gl::TexImage2D(gl::TEXTURE_2D,
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
                        gl::TexImage2D(gl::TEXTURE_2D,
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
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

            gl::BindTexture(gl::TEXTURE_2D, 0);
            debug_assert_eq!(gl::GetError(), gl::NO_ERROR);

            Ok(SurfaceTexture { surface, gl_texture, phantom: PhantomData })
        }
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // Avoid a panic and just leak the surface.
            surface.glx_pixmap = 0;
            return Err(Error::IncompatibleSurface)
        }

        self.make_context_not_current(context)?;

        unsafe {
            glXDestroyPixmap(self.native_display.display(), surface.glx_pixmap);
            surface.glx_pixmap = 0;
        }

        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, surface_texture.gl_texture);

            if !self.quirks.contains(Quirks::BROKEN_GLX_TEXTURE_FROM_PIXMAP) {
                let display = self.native_display.display() as *mut GlxDisplay;
                glx::ReleaseTexImageEXT(display,
                                        surface_texture.surface.glx_pixmap,
                                        glx::FRONT_EXT as c_int);
            }

            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::DeleteTextures(1, &mut surface_texture.gl_texture);
            surface_texture.gl_texture = 0;
        }

        Ok(surface_texture.surface)
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        SurfaceID(self.glx_pixmap as usize)
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }

    #[inline]
    pub fn gl_texture_target() -> GLenum {
        gl::TEXTURE_2D
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
