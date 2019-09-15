//! Platform-specific backends.

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
pub mod with_glx;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
pub use with_glx as default;

#[cfg(feature="osmesa")]
pub mod with_osmesa;
#[cfg(feature="osmesa")]
pub use with_osmesa as default;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos as default;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::angle as default;

#[cfg(target_os="ios")]
pub mod with_eagl;

pub mod not_implemented;
#[cfg(not(any(unix, target_os="windows")))]
pub use not_implemented as default;
