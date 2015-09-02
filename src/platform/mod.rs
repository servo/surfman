#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeDisplay;

pub trait NativeGLContextMethods: Sized {
    fn get_proc_address(&str) -> *const ();

    fn create_headless() -> Result<Self, &'static str>;
    fn is_current(&self) -> bool;
    fn make_current(&self) -> Result<(), &'static str>;
    fn unbind(&self) -> Result<(), &'static str>;

    #[cfg(feature="texture_surface")]
    fn get_display(&self) -> NativeDisplay;
}

#[cfg(target_os="linux")]
pub mod with_glx;

#[cfg(target_os="linux")]
pub use self::with_glx::NativeGLContext;

#[cfg(target_os="macos")]
pub mod with_cgl;

#[cfg(target_os="macos")]
pub use self::with_cgl::NativeGLContext;

#[cfg(target_os="android")]
pub mod with_egl;

#[cfg(target_os="android")]
pub use self::with_egl::NativeGLContext;

// TODO(ecoal95): Get a machine to test with mac and
// get android building:
//
// #[cfg(not(target_os="linux"))]
// pub mod with_egl;
//
// #[cfg(not(target_os="linux"))]
// pub use platform::with_egl::NativeGLContext;

