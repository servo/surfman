// surfman/surfman/src/platform/generic/mod.rs
//
//! Backends that are not specific to any operating system.

#[cfg(any(android_platform, angle, free_unix, ohos_platform))]
pub(crate) mod egl;

pub mod multi;
