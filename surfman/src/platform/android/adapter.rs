// surfman/surfman/src/platform/android/adapter.rs
//
//! Android graphics adapters.
//!
//! This is presently a no-op. In the future we might want to support the
//! `EGLDeviceEXT` extension for multi-GPU setups.

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
    /// The Android backend has no software support, so this returns an error. You can use the
    /// universal backend to get a software adapter.
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }
}
