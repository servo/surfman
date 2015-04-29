use geom::Size2D;

pub trait NativeGLContextMethods {
    // TODO(ecoal95): create_headless should not require a size
    fn create_headless(Size2D<i32>) -> Result<Self, &'static str>;
    fn is_current(&self) -> bool;
    fn make_current(&self) -> Result<(), &'static str>;

    #[cfg(target_os="android")]
    fn is_gles() -> bool {
        true
    }

    #[cfg(not(target_os="android"))]
    fn is_gles() -> bool {
        false
    }
}

#[cfg(target_os="linux")]
pub mod with_glx;

#[cfg(target_os="linux")]
pub use self::with_glx::NativeGLContext;

#[cfg(not(target_os="linux"))]
pub mod not_implemented;

#[cfg(not(target_os="linux"))]
pub use self::not_implemented::NativeGLContext;

// TODO(ecoal95): Get a machine to test with mac and
// get android building:
//
// #[cfg(not(target_os="linux"))]
// pub mod with_egl;
//
// #[cfg(not(target_os="linux"))]
// pub use platform::with_egl::NativeGLContext;

