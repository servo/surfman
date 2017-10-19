use gleam::gl::types::GLenum;
use gleam::gl;
use GLContextAttributes;
use GLVersion;

/// This structure is here to allow
/// cross-platform formatting
pub struct GLFormats {
    pub color_renderbuffer: GLenum,
    pub texture_internal: GLenum,
    pub texture: GLenum,
    pub texture_type: GLenum,
    pub depth: GLenum,
    pub stencil: GLenum,
    pub packed_depth_stencil: bool,
}

impl GLFormats {
    // In the future we may use extension detection et-al to improve this, for now
    // platform dependent.
    //
    // FIXME: In linux with GLES2 texture attachments create INVALID_ENUM errors.
    // I suspect that it's because of texture formats, but I need time to debugit.
    #[cfg(not(target_os="android"))]
    pub fn detect(attrs: &GLContextAttributes, extensions: &[String], api_version: GLVersion) -> GLFormats {
        let packed_depth_stencil = GLFormats::supports_packed_depth_stencil(&extensions, api_version);

        if attrs.alpha {
            GLFormats {
                color_renderbuffer: gl::RGBA8,
                texture_internal: gl::RGBA,
                texture: gl::RGBA,
                texture_type: gl::UNSIGNED_BYTE,
                depth: gl::DEPTH_COMPONENT24,
                stencil: gl::STENCIL_INDEX8,
                packed_depth_stencil: packed_depth_stencil,
            }
        } else {
            GLFormats {
                color_renderbuffer: gl::RGB8,
                texture_internal: gl::RGB8,
                texture: gl::RGB,
                texture_type: gl::UNSIGNED_BYTE,
                depth: gl::DEPTH_COMPONENT24,
                stencil: gl::STENCIL_INDEX8,
                packed_depth_stencil: packed_depth_stencil,
            }
        }
    }

    #[cfg(target_os="android")]
    pub fn detect(attrs: &GLContextAttributes, extensions: &[String], api_version: GLVersion) -> GLFormats {
        // detect if the GPU supports RGB8 and RGBA8 renderbuffer/texture storage formats.
        // GL_ARM_rgba8 extension is similar to OES_rgb8_rgba8, but only exposes RGBA8.
        let has_rgb8 = extensions.iter().any(|s| s == "GL_OES_rgb8_rgba8");
        let has_rgba8 = has_rgb8 || extensions.iter().any(|s| s == "GL_ARM_rgba8");

        let packed_depth_stencil = GLFormats::supports_packed_depth_stencil(&extensions, api_version);

        if attrs.alpha {
            GLFormats {
                color_renderbuffer: if has_rgba8 { gl::RGBA8 } else { gl::RGBA4 },
                texture_internal: gl::RGBA,
                texture: gl::RGBA,
                texture_type: if has_rgba8 { gl::UNSIGNED_BYTE } else { gl::UNSIGNED_SHORT_4_4_4_4 },
                depth: gl::DEPTH_COMPONENT16,
                stencil: gl::STENCIL_INDEX8,
                packed_depth_stencil: packed_depth_stencil,
            }
        } else {
            GLFormats {
                color_renderbuffer: if has_rgb8 { gl::RGB8 } else { gl::RGB565 },
                texture_internal: gl::RGB,
                texture: gl::RGB,
                texture_type: if has_rgb8 { gl::UNSIGNED_BYTE } else { gl::UNSIGNED_SHORT_4_4_4_4 },
                depth: gl::DEPTH_COMPONENT16,
                stencil: gl::STENCIL_INDEX8,
                packed_depth_stencil: packed_depth_stencil,
            }
        }
    }

    // Extension detection check to avoid incomplete framebuffers when using both depth and stencil buffers.
    // Some implementations don't support separated DEPTH_COMPONENT and STENCIL_INDEX8 renderbuffers.
    // Other implementations support only DEPTH24_STENCIL8 renderbuffer attachments.
    fn supports_packed_depth_stencil(extensions: &[String], api_version: GLVersion) -> bool {
        if api_version.major_version() >= 3 {
            // packed depth stencil is included in OpenGL Core 3.x.
            // It may not be available in the extension list (e.g. MacOS)
            return true;
        }
        extensions.iter().any(|s| s == "GL_OES_packed_depth_stencil" || s == "GL_EXT_packed_depth_stencil")
    }
}

