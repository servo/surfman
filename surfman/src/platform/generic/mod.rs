// surfman/surfman/src/platform/generic/mod.rs
//
//! Backends that are not specific to any operating system.

#[cfg(any(android, angle, linux))]
pub(crate) mod egl;

pub mod multi;
