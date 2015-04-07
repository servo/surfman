#[cfg(target_os="linux")]
pub mod glx;

#[cfg(target_os="linux")]
pub use platform::glx::gl_context;

#[cfg(not(target_os="linux"))]
// pub mod not_implemented;

#[cfg(not(target_os="linux"))]
pub use not_implemented::gl_context;
