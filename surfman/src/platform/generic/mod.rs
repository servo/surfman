// surfman/src/platform/generic/mod.rs

#[cfg(any(target_os = "android", target_os = "windows"))]
pub(crate) mod egl;

#[cfg(feature = "sm-osmesa")]
pub mod osmesa;

#[cfg(all(feature = "sm-osmesa", not(target_os = "windows")))]
pub mod universal;
#[cfg(target_os = "windows")]
pub use crate::platform::windows::angle as universal;
