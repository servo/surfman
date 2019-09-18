//! OpenGL function pointer loading.

use crate::glx;

use std::os::raw::c_void;

pub(crate) fn load_with<F>(loader: F) where F: FnMut(&'static str) -> *const c_void {
    glx::load_with(loader);
}
