// surfman/surfman/src/platform/generic/mod.rs
//
//! Backends that are not specific to any operating system.

#[cfg(any(target_os = "android",
          all(target_os = "windows", feature = "sm-angle"),
          all(unix, not(target_os = "macos"))))]
pub(crate) mod egl;

pub mod multi;
