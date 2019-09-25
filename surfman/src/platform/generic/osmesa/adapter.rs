//! A wrapper for OSMesa adapters. These are a no-op, since OSMesa is
//! generally built with a single driver.

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
    ///
    /// OSMesa is a software backend, so this returns an error.
    #[inline]
    pub fn hardware() -> Result<Adapter, Error> {
        Err(Error::NoHardwareAdapters)
    }

    /// Returns the "best" software adapter on this system.
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Adapter::default()
    }
}
