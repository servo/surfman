// surfman/src/platform/src/windows/wgl/adapter.rs
//
//! A no-op adapter type for WGL.
//!
//! TODO(pcwalton): Try using one of the multi-GPU extensions for this.

use crate::Error;

/// A no-op adapter.
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
    /// The WGL backend has no software support, so this returns an error. You can use the
    /// universal backend to get a software adapter.
    ///
    /// FIXME(pcwalton): Does it really?
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }
}
