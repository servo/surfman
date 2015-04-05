#![feature(unsafe_destructor)]

extern crate xlib;
extern crate glx;
extern crate gleam;
extern crate libc;

pub mod platform;

trait GLContextMethods {
    fn create_headless() -> Result<Self, &'static str>;
    fn create_offscreen() -> Result<Self, &'static str>;
    fn make_current(&self) -> Result<(), &'static str>;
}
