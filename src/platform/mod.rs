#[cfg(target_os="linux")]
mod glx;

#[cfg(target_os="linux")]
pub use platform::glx::gl_context;

#[cfg(not(target_os="linux"))]
pub use not_implemented::gl_context;
