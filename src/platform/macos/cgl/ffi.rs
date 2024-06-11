// surfman/surfman/src/platform/macos/cgl/ffi.rs
//
//! FFI declarations not provided by the upstream `cgl` crate.

use cgl::CGLContextObj;

#[link(name = "OpenGL", kind = "framework")]
extern "C" {
    pub(crate) fn CGLRetainContext(ctx: CGLContextObj) -> CGLContextObj;
    pub(crate) fn CGLReleaseContext(ctx: CGLContextObj);
}
