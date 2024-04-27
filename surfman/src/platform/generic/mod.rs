// surfman/surfman/src/platform/generic/mod.rs
//
//! Backends that are not specific to any operating system.

#[cfg(any(android_platform, angle, free_unix))]
pub(crate) mod egl;

pub mod multi;
