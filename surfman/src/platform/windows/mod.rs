// surfman/src/platform/windows/mod.rs

#[cfg(feature = "sm-angle")]
pub mod angle;

#[cfg(not(feature = "sm-no-wgl"))]
pub mod wgl;
