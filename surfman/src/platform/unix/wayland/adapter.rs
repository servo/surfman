// surfman/surfman/src/platform/unix/wayland/adapter.rs
//
//! A wrapper for Wayland adapters.
//! 
//! TODO(pcwalton): There may be better Wayland extensions we can use for this.

#[derive(Clone, Debug)]
pub enum Adapter {
    Hardware,
    Software,
}

