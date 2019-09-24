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
}
