#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
}

#[cfg(feature = "serde")]
impl Deserialize for GLContextAttributes {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let alpha = try!(bool::deserialize(deserializer));
        let depth = try!(bool::deserialize(deserializer));
        let stencil = try!(bool::deserialize(deserializer));
        let antialias = try!(bool::deserialize(deserializer));
        let premultiplied_alpha = try!(bool::deserialize(deserializer));
        let preserve_drawing_buffer = try!(bool::deserialize(deserializer));
        Ok(GLContextAttributes {
            alpha: alpha,
            depth: depth,
            stencil: stencil,
            antialias: antialias,
            premultiplied_alpha: premultiplied_alpha,
            preserve_drawing_buffer: preserve_drawing_buffer,
        })
    }
}

#[cfg(feature = "serde")]
impl Serialize for GLContextAttributes {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        try!(self.alpha.serialize(serializer));
        try!(self.depth.serialize(serializer));
        try!(self.stencil.serialize(serializer));
        try!(self.antialias.serialize(serializer));
        try!(self.premultiplied_alpha.serialize(serializer));
        try!(self.preserve_drawing_buffer.serialize(serializer));
        Ok(())
    }
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


