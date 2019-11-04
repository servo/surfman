// surfman/surfman/src/platform/unix/x11/adapter.rs
//
//! A wrapper for X11 adapters.
//! 
//! These are no-ops, since we don't support multi-GPU on X11 yet.

use crate::Error;

#[derive(Clone, Debug)]
pub struct Adapter;

impl Adapter {
    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn default() -> Result<Adapter, Error> {
        Ok(Adapter)
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn hardware() -> Result<Adapter, Error> {
        Adapter::default()
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The X11 backend has no software support, so this returns an error.
    ///
    /// TODO(pcwalton): If Mesa is in use, maybe we could use `llvmpipe` somehow?
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }
}
