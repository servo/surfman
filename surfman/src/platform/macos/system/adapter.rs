// surfman/surfman/src/platform/macos/system/adapter.rs
//
//! A wrapper for Metal adapters.

/// An adapter.
#[derive(Clone, Debug)]
pub struct Adapter {
    pub(crate) is_low_power: bool,
}
