
/// This structure represents the attributes the context must support
/// It's almost (if not) identical to WebGLGLContextAttributes
#[derive(Clone, Debug, Copy)]
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

    // FIXME(ecoal95): `antialias` should be true by default
    //   but we do not support antialising so... We must change it
    //   when we do. See GLFeature.
    pub fn default() -> GLContextAttributes {
        GLContextAttributes {
            alpha: true,
            depth: true,
            stencil: false,
            antialias: false,
            premultiplied_alpha: true,
            preserve_drawing_buffer: false
        }
    }
}


