// surfman/surfman/src/platform/generic/multi/mod.rs
//
//! An abstraction that allows the choice of backends dynamically.
//!
//! This is useful on Unix systems, because it allows for Wayland to be tried first, and, failing
//! that, to use X11.
//!
//! Each type here has two type parameters: a "default" device and an "alternate" device. Opening a
//! connection will first attempt to open the default connection and, if that fails, attempts to
//! open the alternate connection. You can also create instances of these types manually (i.e.
//! wrapping a default or alternate type directly) if you have platform-specific initialization
//! code.
//!
//! You can "daisy chain" these types to switch between more than two backends. For example, you
//! might use `multi::Device<wayland::Device, multi::Device<x11::Device, osmesa::Device>>` for a
//! device that can dynamically switch between Wayland, X11, and OSMesa.

pub mod connection;
pub mod context;
pub mod device;
pub mod surface;
