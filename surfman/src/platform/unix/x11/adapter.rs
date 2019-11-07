// surfman/surfman/src/platform/unix/x11/adapter.rs
//
//! A wrapper for X11 adapters.
//! 
//! These are no-ops, since we don't support multi-GPU on X11 yet.

#[derive(Clone, Debug)]
pub enum Adapter {
    Hardware,
    Software,
}
