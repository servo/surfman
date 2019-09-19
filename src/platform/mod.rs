//! Platform-specific backends.

#[cfg(feature="osmesa")]
pub mod with_osmesa;
#[cfg(feature="osmesa")]
pub use with_osmesa as default;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(all(target_os = "macos", not(feature = "sm-x11")))]
pub use macos as default;

#[cfg(any(feature = "sm-x11", all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub mod unix;
#[cfg(any(feature = "sm-x11", all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub use unix::x11 as default;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::angle as default;

#[cfg(target_os="ios")]
pub mod with_eagl;
