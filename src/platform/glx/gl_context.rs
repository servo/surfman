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

// impl GLContextMethods for GLContext {}
