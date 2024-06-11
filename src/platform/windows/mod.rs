// surfman/surfman/src/platform/windows/mod.rs
//
//! Windows support, either via the native WGL interface or Google's ANGLE library.

#[cfg(feature = "sm-angle")]
pub mod angle;

#[cfg(not(feature = "sm-no-wgl"))]
pub mod wgl;
