// surfman/src/platform/unix/mod.rs

#[cfg(any(target_os = "android", all(target_os = "windows", not(feature = "sm-osmesa"))))]
pub(crate) mod egl;

#[cfg(feature = "sm-osmesa")]
pub mod osmesa;
