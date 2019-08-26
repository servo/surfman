use crate::gl_context::{GLContextDispatcher, GLVersion};
use gleam::gl;

pub enum DefaultSurfaceSwapResult {
    Swapped { old_surface: NativeSurface },
    NotSupported { new_surface: NativeSurface },
    Failed { message: &'static str, new_surface: NativeSurface },
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
pub mod with_glx;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios")), feature="x11"))]
pub use self::with_glx::{NativeGLContext, NativeGLContextHandle};

#[cfg(feature="osmesa")]
pub mod with_osmesa;
#[cfg(feature="osmesa")]
pub use self::with_osmesa::{OSMesaContext, OSMesaContextHandle};
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android")), not(feature="x11")))]
pub use self::with_osmesa::{OSMesaContext as NativeGLContext, OSMesaContextHandle as NativeGLContextHandle};


#[cfg(any(
    target_os="android",
    all(target_os="windows", feature="no_wgl"),
    all(target_os="linux", feature = "test_egl_in_linux"),
))]
pub mod with_egl;
#[cfg(any(target_os="android", all(target_os="windows", feature="no_wgl")))]
pub use self::with_egl::{Display, NativeGLContext, NativeGLContextHandle};
#[cfg(any(target_os="android", all(target_os="windows", feature="no_wgl")))]
pub use self::with_egl::{NativeSurface, NativeSurfaceTexture};

#[cfg(target_os="macos")]
pub mod with_cgl;
#[cfg(target_os="macos")]
pub use self::with_cgl::{Display, NativeDisplay, NativeGLContext};
#[cfg(target_os="macos")]
pub use self::with_cgl::{NativeSurface, NativeSurfaceTexture};

#[cfg(all(target_os="windows", not(feature="no_wgl")))]
pub mod with_wgl;
#[cfg(all(target_os="windows", not(feature="no_wgl")))]
pub use self::with_wgl::NativeGLContext;

#[cfg(target_os="ios")]
pub mod with_eagl;
#[cfg(target_os="ios")]
pub use self::with_eagl::NativeGLContext;

pub mod not_implemented;
#[cfg(not(any(unix, target_os="windows")))]
pub use self::not_implemented::NativeGLContext;
#[cfg(not(any(target_os="macos",
              target_os="android",
              all(target_os="windows", feature="no_wgl"))))]
pub use self::not_implemented::{NativeSurface, NativeSurfaceTexture};
