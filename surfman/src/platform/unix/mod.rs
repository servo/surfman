// surfman/src/platform/unix/mod.rs

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod wayland;
#[cfg(all(any(feature = "sm-x11",
              all(unix, not(any(target_os = "macos", target_os = "android"))))))]
pub mod x11;
