//! OpenGL function pointer loading.

use std::os::raw::c_void;

pub(crate) fn load_with<F>(_: F) where F: FnMut(&'static str) -> *const c_void {}
