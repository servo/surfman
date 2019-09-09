//! Cached OpenGL information.

use crate::gl_limits::GLLimits;
use gl;
use std::ffi::CStr;
use std::os::raw::c_char;

bitflags! {
    // https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
    pub struct ContextAttributeFlags: u8 {
        const ALPHA   = 0x01;
        const DEPTH   = 0x02;
        const STENCIL = 0x04;
    }
}

bitflags! {
    pub struct FeatureFlags: u8 {
        const SUPPORTS_DEPTH24_STENCIL8 = 0x01;
    }
}

/// The OpenGL API and its associated version.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GLFlavor {
    pub api: GLApi,
    pub version: GLVersion,
}

// https://www.khronos.org/registry/webgl/specs/latest/1.0/#WEBGLCONTEXTATTRIBUTES
#[derive(Clone, Copy)]
pub struct ContextAttributes {
    pub flavor: GLFlavor,
    pub flags: ContextAttributeFlags,
}

/// Information about the OpenGL implementation and context in use.
///
/// This data is cached.
#[derive(Clone, Copy)]
pub struct GLInfo {
    pub attributes: ContextAttributes,
    pub limits: GLLimits,
    pub features: FeatureFlags,
}

impl GLInfo {
    // Creates a placeholder `GLInfo`. It must be populated afterward with the `populate()` method.
    pub(crate) fn new(attributes: &ContextAttributes) -> GLInfo {
        GLInfo {
            attributes: *attributes,
            limits: GLLimits::default(),
            features: FeatureFlags::empty(),
        }
    }

    // Assumes that the context with the given attributes is current.
    pub(crate) fn populate(&mut self) {
        self.limits = GLLimits::detect();
        self.features = FeatureFlags::detect(&self.attributes);
    }
}

impl FeatureFlags {
    // Assumes that the context with the given attributes is current.
    fn detect(attributes: &ContextAttributes) -> FeatureFlags {
        let mut flags = FeatureFlags::empty();
        let extensions = GLExtensions::detect(attributes);

        // Packed depth/stencil is included in OpenGL Core 3.x.
        // It may not be available in the extension list (e.g. on macOS).
        if attributes.flavor.version.major >= 3 ||
                extensions.0.iter().any(|name| {
                    name == "GL_OES_packed_depth_stencil" || name == "GL_EXT_packed_depth_stencil"
                }) {
            flags.insert(FeatureFlags::SUPPORTS_DEPTH24_STENCIL8);
        }

        flags
    }
}

/// The API (OpenGL or OpenGL ES).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GLApi {
    GL,
    GLES,
}

/// Describes the OpenGL version that is requested when a context is created.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GLVersion {
    pub major: u8,
    pub minor: u8,
}

impl GLVersion {
    #[inline]
    pub fn new(major: u8, minor: u8) -> GLVersion {
        GLVersion { major, minor }
    }
}

#[derive(Debug)]
struct GLExtensions(Vec<String>);

impl GLExtensions {
    // Assumes that the context with the given attributes is current.
    fn detect(attributes: &ContextAttributes) -> GLExtensions {
        unsafe {
            if attributes.flavor.version.major < 3 {
                let extensions = gl::GetString(gl::EXTENSIONS) as *const c_char;
                let extensions = CStr::from_ptr(extensions).to_string_lossy();
                let extensions = extensions.split(&[',', ' '][..]);
                return GLExtensions(extensions.map(|string| string.to_string()).collect());
            }

            let mut extension_count = 0;
            gl::GetIntegerv(gl::NUM_EXTENSIONS, &mut extension_count);

            let mut extensions = Vec::with_capacity(extension_count as usize);
            for extension_index in 0..extension_count {
                let extension = gl::GetStringi(gl::EXTENSIONS, extension_index as u32);
                extensions.push(CStr::from_ptr(extension as *const c_char).to_string_lossy().to_string());
            }

            GLExtensions(extensions)
        }
    }
}
