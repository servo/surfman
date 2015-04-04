use glx;
use xlib::*;
use glx::types::{GLXPixmap};
use libc::*;
use gleam::gl;
use GLContextMethods;

pub struct GLContext {
	native : XID
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
fn create_offscreen_rendering_context(width: u32, height: u32) -> Result<GLXPixmap, &'static str> {
	let dpy = unsafe { XOpenDisplay(0 as *mut c_char) as *mut Display };

	let attributes: [c_int] = [
		glx::DRAWABLE_TYPE,
		glx::PIXMAP_BIT,
		glx::X_RENDERABLE, 1,
		glx::NONE
	];

	let mut config_count : c_int = 0;

	let configs = unsafe { glx::ChooseFBConfig(dpy as *mut glx::types::Display, XDefaultScreen(dpy), attributes.as_mut() as *mut c_int, &config_count as *mut c_int) };

	if configs.is_null() {
		return Err("glx::ChooseFBConfig");
	}

	debug_assert!(config_count > 0);

	let mut config_index = 0;
	let mut visual_id = glx::NONE;
	for i in 0..config_count {
		unsafe {
			let config = configs.offset(i);
			let drawable_type : c_int = 0;

			// glx's `Success` is unreachable from bindings, but it's defined to 0
			if glx::GetFBConfigAttrib(dpy as *mut glx::types::Display, config, glx::VISUAL_ID, &drawable_type as *mut c_int) != 0 || ! (drawable_type & glx::PIXMAP_BIT ) {
				continue;
			}

			if glx::GetFBConfigAttrib(dpy as *mut glx::types::Display, config, glx::VISUAL_ID, &visual_id as *mut c_int) != 0 || visual_id == 0 {
				continue;
			}
		}

		config_index = i;
		break;
	}

	if visual_id == 0 {
		unsafe { XFree(configs) };
		return Err("We don't have any config with visuals");
	}

	let screen = unsafe { XDefaultScreenOfDisplay(dpy) };

	// TODO: Get visual and depth from visual id... Undoable without access to Screen* structure?
	let (visual, depth) = try!(unsafe { get_visual_and_depth(screen, visual_id) });

	let pixmap = unsafe { XCreatePixmap(dpy, XRootWindowOfScreen(screen), width, height, depth) };

	if pixmap == 0 {
		unsafe { XFree(configs) };
		return Err("XCreatePixMap");
	}

	let glx_pixmap = unsafe { glx::CreatePixmap(dpy, *configs.offset(config_index), pixmap, 0 as *mut c_void) };

	unsafe { XFree(configs) };

	if glx_pixmap == 0 {
		return Err("glx::createPixmap");
	}

	Ok(glx_pixmap)
}

impl GLContextMethods for GLContext {
	pub fn create_offscreen() -> Result<GLContext, &'static str> {
		let dpy = unsafe { XOpenDisplay(0 as *mut c_char) };

		if dpy.is_null() {
			return Err("XOpenDisplay");
		}

		let visual = unsafe {
			// TODO: allow options to choose more capabilities?
			let attributes : [c_uint] = [
				glx::RGBA,
				glx::DEPTH_SIZE, 1,
				glx::RED_SIZE, 1,
				glx::GREEN_SIZE, 1,
				glx::BLUE_SIZE, 1,
				glx::NONE
			];
			glx::ChooseVisual(dpy as, XDefaultRootWindow(dpy), attributes)
		};

		if visual.is_null() {
			return Err("glXChooseVisual");
		}

		Err("Unimplemented")
	}
}
