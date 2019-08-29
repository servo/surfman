//! A stub implementation of an OpenGL context that reports errors when methods are invoked on it.

use crate::{Error, GLInfo};
use super::device::Device;
use super::surface::Surface;
use gleam::gl::Gl;
use std::os::raw::c_void;

pub struct Context;

impl Device {
    pub fn create_context(&self, _: &dyn Gl, _: &GLInfo) -> Result<Context, Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }

    pub fn destroy_context(&self, _: &mut Context, _: &dyn Gl) -> Result<(), Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }

    pub fn make_context_current(&self, _: &mut Context) -> Result<(), Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }

    pub fn make_context_not_current(&self, _: &mut Context) -> Result<(), Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }

    pub fn get_proc_address(&self, _: &mut Context, _: &str)
                            -> Result<*const c_void, Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }

    pub fn replace_color_surface(&self, _: &dyn Gl, _: &mut Context, _: Surface)
                                 -> Result<Option<Surface>, Error> {
        Err(Error::UnsupportedOnThisPlatform)
    }
}
