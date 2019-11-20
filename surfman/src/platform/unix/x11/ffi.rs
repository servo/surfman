// surfman/surfman/platform/unix/x11/ffi.rs
//
//! Extra FFI declarations.

use std::os::raw::c_int;

pub(crate) const GLX_CONTEXT_PROFILE_MASK_ARB: c_int = 0x9126;

pub(crate) const GLX_CONTEXT_CORE_PROFILE_BIT_ARB:          c_int = 1;
pub(crate) const GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: c_int = 2;

