// surfman/surfman/src/platform/unix/wayland/mod.rs
//
//! Bindings to Wayland via the Linux GBM interface.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;

#[path = "../../../implementation/mod.rs"]
mod implementation;

#[cfg(test)]
#[path = "../../../tests.rs"]
mod tests;
