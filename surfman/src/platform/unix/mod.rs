// surfman/surfman/src/platform/unix/mod.rs
//
//! Backends specific to Unix-like systems, particularly Linux.

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod default;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod generic;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod surfaceless;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod wayland;
#[cfg(all(any(feature = "sm-x11",
              all(unix, not(any(target_os = "macos", target_os = "android"))))))]
pub mod x11;
