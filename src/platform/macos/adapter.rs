//! A wrapper for Core OpenGL adapters.

/// A no-op adapter.
#[derive(Clone, Debug)]
pub struct Adapter;

impl Adapter {
    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn default() -> Adapter {
        Adapter
    }
}
