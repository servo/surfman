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
}
