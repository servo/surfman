#[cfg(target_os="linux")]
pub mod with_glx;

#[cfg(target_os="linux")]
pub use platform::with_glx::gl_context::{GLContext};

#[cfg(not(target_os="linux"))]
pub mod not_implemented;

#[cfg(not(target_os="linux"))]
pub use platform::not_implemented::gl_context::{GLContext};

// TODO(ecoal95): Get a machine to test with mac and
// get android building, so one day:
//
// #[cfg(not(target_os="linux"))]
// pub mod with_egl;
//
// #[cfg(not(target_os="linux"))]
// pub use platform::with_egl::gl_context::{GLContext};

