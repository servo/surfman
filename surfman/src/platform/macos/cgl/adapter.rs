// surfman/surfman/src/platform/src/macos/cgl/adapter.rs
//
//! A wrapper for Core OpenGL adapters.

use crate::platform::macos::system::adapter::Adapter as SystemAdapter;

/// A no-op adapter.
#[derive(Clone, Debug)]
pub struct Adapter(pub SystemAdapter);
