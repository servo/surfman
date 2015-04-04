extern crate xlib;
extern crate glx;
extern crate gleam;
extern crate libc;

pub mod platform;

trait GLContextMethods {
	pub fn create_offscreen() -> GLContextMethods;
	pub fn make_current(&self);
}
