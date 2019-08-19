/// WGL bindings
pub mod wgl {
    include!(concat!(env!("OUT_DIR"), "/wgl_bindings.rs"));
}

/// Functions that are not necessarly always available
pub mod wgl_ext {
    include!(concat!(env!("OUT_DIR"), "/wgl_extra_bindings.rs"));
}

mod wgl_attributes;
mod gl_context;
mod utils;
pub use self::gl_context::{NativeGLContext, NativeGLContextHandle};
