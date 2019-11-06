// surfman/src/platform/src/windows/wgl/adapter.rs
//
//! A no-op adapter type for WGL.
//!
//! TODO(pcwalton): Try using one of the multi-GPU extensions for this.

/// A no-op adapter.
#[derive(Clone, Debug)]
pub struct Adapter;

