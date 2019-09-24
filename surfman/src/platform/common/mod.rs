// surfman/src/platform/common/mod.rs

//! Miscellaneous functionality shared among some, but not all backends.

#[cfg(any(target_os = "android", all(target_os = "windows", not(feature = "sm-osmesa"))))]
pub(crate) mod egl;
