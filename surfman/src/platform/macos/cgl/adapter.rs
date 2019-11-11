// surfman/surfman/src/platform/macos/cgl/adapter.rs
//
//! A wrapper for Core OpenGL adapters.

use crate::platform::macos::system::adapter::Adapter as SystemAdapter;

/// Represents a display adapter on macOS.
/// 
/// Adapters can be sent between threads. You can use them with a `Connection` to open the device.
#[derive(Clone, Debug)]
pub struct Adapter(pub(crate) SystemAdapter);
