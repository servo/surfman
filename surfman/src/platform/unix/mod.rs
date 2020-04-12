// surfman/surfman/src/platform/unix/mod.rs
//
//! Backends specific to Unix-like systems, particularly Linux.

// The default when x11 is enabled
#[cfg(x11)]
pub mod default;

// The default when x11 is not enabled
#[cfg(not(x11))]
pub use wayland as default;

#[cfg(linux)]
pub mod generic;

#[cfg(linux)]
pub mod wayland;
#[cfg(x11)]
pub mod x11;
