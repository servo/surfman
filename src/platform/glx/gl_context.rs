use glx;
use xlib::*;
use glx::types::{GLXPixmap};
use libc::*;
use gleam::gl;
use GLContextMethods;

pub struct GLContext {
	display: *mut c_void,
	native: XID
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
fn create_offscreen_pixmap_content(width: u32, height: u32) -> Result<GLXPixmap, &'static str> {
	let dpy = unsafe { XOpenDisplay(0 as *mut c_char) };

	let attributes = vec![
		glx::DRAWABLE_TYPE as c_int,
		glx::PIXMAP_BIT as c_int,
		glx::X_RENDERABLE as c_int, 1,
		glx::NONE as c_int
	];

	let mut config_count : c_int = 0;

	let configs = unsafe { glx::ChooseFBConfig(dpy as *mut glx::types::Display, XDefaultScreen(dpy), attributes.as_mut_ptr(), &mut config_count as *mut c_int) };

	if configs.is_null() {
		return Err("glx::ChooseFBConfig");
	}

	debug_assert!(config_count > 0);

	let mut config_index = 0;
	let mut visual_id = glx::NONE as c_int;
	for i in 0..(config_count as isize) {
		unsafe {
			let config = configs.offset(i) as *const c_void;
			let drawable_type : c_int = 0;

			// glx's `Success` is unreachable from bindings, but it's defined to 0
			if glx::GetFBConfigAttrib(dpy as *mut glx::types::Display, config, glx::VISUAL_ID as c_int, &mut drawable_type as *mut c_int) != 0 || (drawable_type & (glx::PIXMAP_BIT as c_int) == 0) {
				continue;
			}

			if glx::GetFBConfigAttrib(dpy as *mut glx::types::Display, config, glx::VISUAL_ID as c_int, &mut visual_id as *mut c_int) != 0 || visual_id == 0 {
				continue;
			}
		}

		config_index = i;
		break;
	}

	if visual_id == 0 {
		// TODO: bypass type checking
		//  expected `*mut libc::types::common::c95::c_void`,
		//     found `*mut libc::types::common::c95::c_void`
		// (expected enum `libc::types::common::c95::c_void`,
		//     found a different enum `libc::types::common::c95::c_void`) [E0308]
		unsafe { XFree(configs as *mut c_void) };
		return Err("We don't have any config with visuals");
	}

	let screen = unsafe { XDefaultScreenOfDisplay(dpy) };

	// TODO: Get visual and depth from visual id... Undoable without access to Screen* structure?
	let (visual, depth) = try!(unsafe { get_visual_and_depth(screen, visual_id as VisualID) });

	let pixmap = unsafe { XCreatePixmap(dpy, XRootWindowOfScreen(screen), width, height, depth as c_uint) };

	if pixmap == 0 {
		unsafe { XFree(configs as *mut c_void) };
		return Err("XCreatePixMap");
	}

	let glx_pixmap = unsafe { glx::CreatePixmap(dpy as *mut glx::types::Display, *configs.offset(config_index), pixmap, 0 as *const c_int) };

	unsafe { XFree(configs as *mut c_void) };

	if glx_pixmap == 0 {
		return Err("glx::createPixmap");
	}

	Ok(glx_pixmap)
}
