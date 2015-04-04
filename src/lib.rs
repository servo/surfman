extern crate xlib;
extern crate glx;
extern crate gleam;
extern crate libc;

pub mod platform;

trait GLContextMethods {
    fn create_offscreen() -> GLContextMethods;
    fn make_current(&self);
}
