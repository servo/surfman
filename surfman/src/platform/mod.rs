// surfman/surfman/src/platform/mod.rs
//
//! Platform-specific backends.

pub mod generic;
#[cfg(feature = "sm-osmesa-default")]
pub use generic::osmesa as default;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android as default;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(all(target_os = "macos", not(any(feature = "sm-x11", feature = "sm-osmesa-default"))))]
pub use macos::cgl as default;
#[cfg(target_os = "macos")]
pub use macos::system;

#[cfg(unix)]
pub mod unix;
#[cfg(all(unix,
          not(target_os = "macos"),
          not(target_os = "android"),
          not(feature = "sm-osmesa-default")))]
pub use unix::default;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(all(target_os = "windows", feature = "sm-angle-default"))]
pub use windows::angle as default;
#[cfg(all(target_os = "windows",
          not(any(feature = "sm-osmesa-default", feature = "sm-angle-default"))))]
pub use windows::wgl as default;
