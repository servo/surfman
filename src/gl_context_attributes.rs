
/// This structure represents the attributes the context must support
/// It's almost (if not) identical to WebGLGLContextAttributes
pub struct GLContextAttributes {
    pub alpha: bool,
    pub depth: bool,
    pub stencil: bool,
    pub antialias: bool,
    pub premultiplied_alpha: bool,
    pub preserve_drawing_buffer: bool,
    // TODO: Some Android devices dont't support
    //   32 bits per pixel, eventually we may want
    //   to allow it
}

impl GLContextAttributes {
    pub fn any() -> GLContextAttributes {
        GLContextAttributes {
            alpha: false,
            depth: false,
            stencil: false,
            antialias: false,
            premultiplied_alpha: false,
            preserve_drawing_buffer: false,
        }
    }

    pub fn default() -> GLContextAttributes {
        GLContextAttributes {
            alpha: true,
            depth: true,
            stencil: false,
            antialias: true,
            premultiplied_alpha: true,
            preserve_drawing_buffer: false
        }
    }
}


