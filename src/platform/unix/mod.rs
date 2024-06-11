// surfman/surfman/src/platform/unix/mod.rs
//
//! Backends specific to Unix-like systems, particularly Linux.

// The default when x11 is enabled, and wayland default is not explicitly selected.
#[cfg(all(x11_platform, not(wayland_default)))]
pub mod default;

#[cfg(wayland_default)]
pub use wayland as default;

#[cfg(free_unix)]
pub mod generic;

#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(x11_platform)]
pub mod x11;
