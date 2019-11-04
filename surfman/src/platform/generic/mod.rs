// surfman/src/platform/generic/mod.rs

#[cfg(any(target_os = "android",
          all(target_os = "windows", feature = "sm-angle"),
          all(unix, not(target_os = "macos"))))]
pub(crate) mod egl;

#[cfg(feature = "sm-osmesa")]
pub mod osmesa;

pub mod multi;
