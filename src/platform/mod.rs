// surfman/surfman/src/platform/mod.rs
//
//! Platform-specific backends.

pub mod generic;

#[cfg(any(android_platform, ohos_platform))]
pub mod egl;
#[cfg(any(android_platform, ohos_platform))]
pub use egl as default;

#[cfg(macos_platform)]
pub mod macos;
#[cfg(macos_platform)]
pub use macos::cgl as default;
#[cfg(macos_platform)]
pub use macos::system;

#[cfg(free_unix)]
pub mod unix;
#[cfg(free_unix)]
pub use unix::default;

#[cfg(windows_platform)]
pub mod windows;
#[cfg(angle_default)]
pub use windows::angle as default;
#[cfg(all(windows_platform, not(angle_default)))]
pub use windows::wgl as default;
