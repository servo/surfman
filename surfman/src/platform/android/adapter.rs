// surfman/surfman/src/platform/android/adapter.rs
//
//! Android graphics adapters.
//!
//! This is presently a no-op. In the future we might want to support the
//! `EGLDeviceEXT` extension for multi-GPU setups.

/// A no-op adapter.
#[derive(Clone, Debug)]
pub struct Adapter;
