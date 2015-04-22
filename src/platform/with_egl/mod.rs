mod native_gl_context;
mod utils;
pub use self::native_gl_context::NativeGLContext;

// The last three zeros arw for workaround buggy implementations
macro_rules! egl_end_workarounding_bugs {
    () => {{
        egl::NONE, 0, 0, 0,
    }}
}



