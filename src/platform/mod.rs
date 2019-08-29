//! Platform-specific backends.

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
pub mod with_glx;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
pub use with_glx as default;

#[cfg(feature="osmesa")]
pub mod with_osmesa;
#[cfg(feature="osmesa")]
pub use with_osmesa as default;

#[cfg(any(
    target_os="android",
    all(target_os="windows", feature="no_wgl"),
    all(target_os="linux", feature = "test_egl_in_linux"),
))]
pub mod with_egl;
#[cfg(any(target_os="android", all(target_os="windows", feature="no_wgl")))]
pub use with_egl as default;

#[cfg(target_os="macos")]
pub mod with_cgl;
#[cfg(target_os="macos")]
pub use with_cgl as default;

#[cfg(all(target_os="windows", not(feature="no_wgl")))]
pub mod with_wgl;

#[cfg(target_os="ios")]
pub mod with_eagl;

pub mod not_implemented;
#[cfg(not(any(unix, target_os="windows")))]
pub use not_implemented as default;
