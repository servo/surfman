// surfman/src/platform/generic/mod.rs

#[cfg(any(target_os = "android", target_os = "windows"))]
pub(crate) mod egl;

#[cfg(feature = "sm-osmesa")]
pub mod osmesa;

#[cfg(feature = "sm-osmesa")]
pub mod universal;
