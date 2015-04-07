#![feature(unsafe_destructor)]

extern crate xlib;
extern crate glx;
extern crate gleam;
extern crate libc;
extern crate geom;

use geom::{Size2D};

pub mod platform;
pub mod gl_screen_buffer;

trait GLContextMethods {
    fn create_headless() -> Result<Self, &'static str>;
    fn create_offscreen(Size2D<i32>) -> Result<Self, &'static str>;
    fn make_current(&self) -> Result<(), &'static str>;
}
