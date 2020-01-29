// surfman/surfman/src/platform/mod.rs
//
//! Platform-specific backends.

pub mod generic;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android as default;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(all(target_os = "macos", not(feature = "sm-x11")))]
pub use macos::cgl as default;
#[cfg(target_os = "macos")]
pub use macos::system;

#[cfg(unix)]
pub mod unix;
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
pub use unix::default;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(all(target_os = "windows", feature = "sm-angle-default"))]
pub use windows::angle as default;
#[cfg(all(target_os = "windows", not(feature = "sm-angle-default")))]
pub use windows::wgl as default;
