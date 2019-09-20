//! Declarations common to all platform contexts.

use crate::info::GLVersion;

use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq)]
pub struct ContextID(pub u64);

lazy_static! {
    pub static ref CREATE_CONTEXT_MUTEX: Mutex<ContextID> = Mutex::new(ContextID(0));
}

bitflags! {
    // https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
    pub struct ContextAttributeFlags: u8 {
        const ALPHA   = 0x01;
        const DEPTH   = 0x02;
        const STENCIL = 0x04;
    }
}

// https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
#[derive(Clone, Copy, PartialEq)]
pub struct ContextAttributes {
    pub version: GLVersion,
    pub flags: ContextAttributeFlags,
}

impl ContextAttributes {
    pub(crate) fn zeroed() -> ContextAttributes {
        ContextAttributes { version: GLVersion::new(0, 0), flags: ContextAttributeFlags::empty() }
    }
}
