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
}
