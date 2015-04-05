use glx;
use xlib::*;
use glx::types::{GLXDrawable};
use libc::*;
use platform::glx::gl_context::{GLContext};

struct ScopedXFree<T> {
    ptr: *mut T
}

impl<T> ScopedXFree<T> {
    #[inline(always)]
    fn new(ptr: *mut T) -> ScopedXFree<T> {
        ScopedXFree {
            ptr: ptr
        }
    }

    #[inline(always)]
    fn as_ptr(&self) -> *mut T {
        self.ptr
    }
}

#[unsafe_destructor]
impl<T> Drop for ScopedXFree<T> {
    fn drop(&mut self) {
        unsafe { XFree(self.ptr as *mut c_void); };
    }
}

unsafe fn get_visual_and_depth(s: *mut Screen, id: VisualID) -> Result<(*mut Visual, c_int), &'static str> {
    for d in 0..((*s).ndepths as isize) {
        let depth_info : *mut Depth = (*s).depths.offset(d);
        for v in 0..((*depth_info).nvisuals as isize) {
            let visual : *mut Visual = (*depth_info).visuals.offset(v);
            if (*visual).visualid == id {
                return Ok((visual, (*depth_info).depth));
            }
        }
    }

    Err("Visual not on screen")
}

// Almost directly ported from
// https://dxr.mozilla.org/mozilla-central/source/gfx/gl/GLContextProviderGLX.cpp
pub fn create_offscreen_pixmap_backed_context(width: u32, height: u32) -> Result<GLContext, &'static str> {
    let dpy = unsafe { XOpenDisplay(0 as *mut c_char) };

    let mut attributes = vec![
        glx::DRAWABLE_TYPE as c_int,
        glx::PIXMAP_BIT as c_int,
        glx::X_RENDERABLE as c_int, 1,
        glx::NONE as c_int
    ];

    let mut config_count : c_int = 0;

    let configs = ScopedXFree::new(unsafe {
        glx::ChooseFBConfig(dpy as *mut glx::types::Display, XDefaultScreen(dpy), attributes.as_mut_ptr(), &mut config_count as *mut c_int)
    });

    if configs.as_ptr().is_null() {
        return Err("glx::ChooseFBConfig");
    }

    debug_assert!(config_count > 0);

    let mut config_index = 0;
    let mut visual_id = glx::NONE as c_int;
    for i in 0..(config_count as isize) {
        unsafe {
            let config = *configs.as_ptr().offset(i);
            let mut drawable_type : c_int = 0;

            // glx's `Success` is unreachable from bindings, but it's defined to 0
            if glx::GetFBConfigAttrib(dpy as *mut glx::types::Display, config, glx::VISUAL_ID as c_int, &mut drawable_type as *mut c_int) != 0
                || (drawable_type & (glx::PIXMAP_BIT as c_int) == 0) {
                continue;
            }

            if glx::GetFBConfigAttrib(dpy as *mut glx::types::Display, config, glx::VISUAL_ID as c_int, &mut visual_id as *mut c_int) != 0
                || visual_id == 0 {
                continue;
            }
        }

        config_index = i;
        break;
    }

    if visual_id == 0 {
        return Err("We don't have any config with visuals");
    }

    let screen = unsafe { XDefaultScreenOfDisplay(dpy) };

    let (_, depth) = unsafe { try!(get_visual_and_depth(screen, visual_id as VisualID)) };

    let pixmap = unsafe { XCreatePixmap(dpy, XRootWindowOfScreen(screen), width, height, depth as c_uint) };

    if pixmap == 0 {
        return Err("XCreatePixMap");
    }

    let glx_pixmap = unsafe { glx::CreatePixmap(dpy as *mut glx::types::Display, *configs.as_ptr().offset(config_index), pixmap, 0 as *const c_int) };

    if glx_pixmap == 0 {
        return Err("glx::createPixmap");
    }

    let chosen_config = unsafe { *configs.as_ptr().offset(config_index) };

    GLContext::new(None, true, dpy as *mut glx::types::Display, glx_pixmap as GLXDrawable, chosen_config, true)
}
