// surfman/surfman/src/context.rs
//
//! Declarations common to all platform contexts.

#![allow(unused_imports)]

use crate::gl;
use crate::gl::types::GLuint;
use crate::info::GLVersion;
use crate::Gl;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

/// A unique ID among all currently-allocated contexts.
///
/// If you destroy a context, subsequently-allocated contexts might reuse the same ID.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContextID(pub u64);

lazy_static! {
    #[doc(hidden)]
    pub static ref CREATE_CONTEXT_MUTEX: Mutex<ContextID> = Mutex::new(ContextID(0));
}

bitflags! {
    /// Various flags that control attributes of the context and/or surfaces created from that
    /// context.
    ///
    /// These roughly correspond to:
    /// https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
    ///
    /// There are some extra `surfman`-specific flags as well.
    pub struct ContextAttributeFlags: u8 {
        /// Surfaces created for this context will have an alpha channel (RGBA or BGRA; i.e. 4
        /// channels, 32 bits per pixel, 8 bits per channel). If this is not present, surfaces will
        /// be RGBX or BGRX (i.e. 3 channels, 32 bits per pixel, 8 bits per channel).
        const ALPHA                 = 0x01;
        /// Surfaces created for this context will have a 24-bit depth buffer.
        const DEPTH                 = 0x02;
        /// Surfaces created for this context will have an 8-bit stencil buffer, possibly using
        /// packed depth/stencil if the GL implementation supports it.
        const STENCIL               = 0x04;
        /// The OpenGL compatibility profile will be used. If this is not present, the core profile
        /// is used.
        const COMPATIBILITY_PROFILE = 0x08;
    }
}

/// Attributes that control aspects of a context and/or surfaces created from that context.
///
/// Similar to: https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ContextAttributes {
    /// The OpenGL or OpenGL ES version that this context supports.
    ///
    /// Keep in mind that OpenGL and OpenGL ES have different version numbering schemes. Before
    /// filling in this field, check the result of `Device::gl_api()`.
    pub version: GLVersion,
    /// Various flags.
    pub flags: ContextAttributeFlags,
}

impl ContextAttributes {
    #[allow(dead_code)]
    pub(crate) fn zeroed() -> ContextAttributes {
        ContextAttributes {
            version: GLVersion::new(0, 0),
            flags: ContextAttributeFlags::empty(),
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) fn current_context_uses_compatibility_profile(_gl: &Gl) -> bool {
    false
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
pub(crate) fn current_context_uses_compatibility_profile(gl: &Gl) -> bool {
    unsafe {
        // First, try `GL_CONTEXT_PROFILE_MASK`.
        let mut context_profile_mask = 0;
        gl.GetIntegerv(gl::CONTEXT_PROFILE_MASK, &mut context_profile_mask);
        if gl.GetError() == gl::NO_ERROR
            && (context_profile_mask & gl::CONTEXT_COMPATIBILITY_PROFILE_BIT as i32) != 0
        {
            return true;
        }

        // Second, look for the `GL_ARB_compatibility` extension.
        let mut num_extensions = 0;
        gl.GetIntegerv(gl::NUM_EXTENSIONS, &mut num_extensions);
        if gl.GetError() == gl::NO_ERROR {
            for extension_index in 0..(num_extensions as GLuint) {
                let extension = gl.GetStringi(gl::EXTENSIONS, extension_index) as *const c_char;
                let extension = CStr::from_ptr(extension);
                if extension.to_str() == Ok("GL_ARB_compatibility") {
                    return true;
                }
            }
        }

        false
    }
}
