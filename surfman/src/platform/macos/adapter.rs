// surfman/surfman/src/platform/src/macos/adapter.rs
//
//! A wrapper for Core OpenGL adapters.

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
    /// The macOS backend has no software support, so this returns an error. You can use the
    /// universal backend to get a software adapter.
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }
}
