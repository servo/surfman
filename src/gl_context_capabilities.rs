use gleam::gl;
use gleam::gl::{GLint};
use gl_feature::GLFeature;

/// This is a cross-platform struct, that every GLContext implementation
/// should have under the field `capabilities`, as a public field
/// This should allow us to know the capabilities of a given
/// GLContext without repeating the same code over and over
#[derive(Copy, Clone)]
pub struct GLContextCapabilities {
    // max antialising samples, 0 if no antialising supported
    pub max_samples: GLint,
}

impl GLContextCapabilities {
    pub fn detect() -> GLContextCapabilities {
        let mut capabilities = GLContextCapabilities {
            max_samples: 0,
        };


        if GLFeature::is_supported(GLFeature::FramebufferMultisample) {
            unsafe { gl::GetIntegerv(gl::MAX_SAMPLES, &mut capabilities.max_samples as *mut GLint); };
        }

        capabilities
    }
}
