//! An abstraction that allows the choice of backends dynamically.
//!
//! This is useful on Unix systems, because it allows for Wayland to be tried first, and, failing
//! that, to use X11.

pub mod connection;
