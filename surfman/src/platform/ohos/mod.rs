// surfman/surfman/src/platform/ohos/mod.rs
//
//! Bindings to EGL on OpenHarmony OS.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

mod ffi;

#[path = "../../implementation/mod.rs"]
mod implementation;

#[cfg(feature = "sm-test")]
#[path = "../../tests.rs"]
pub mod tests;
