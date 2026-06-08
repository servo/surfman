//! Shared support code that can be used by multiple Surfman backends.

#[cfg(any(android_platform, angle, free_unix, ohos_platform))]
pub(crate) mod egl;

#[cfg(macos_platform)]
pub(crate) mod io_surface;
