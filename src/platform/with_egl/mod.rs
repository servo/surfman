pub mod gl_context;
pub mod utils;

// The last three zeros arw for workaround buggy implementations
macro_rules! egl_end_workarounding_bugs {
    () => {{
        egl::NONE, 0, 0, 0,
    }}
}



