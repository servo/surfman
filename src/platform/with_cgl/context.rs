//! Wrapper for Core OpenGL contexts.

use cgl::{CGLCreateContext, CGLPixelFormatAttribute};
use core_foundation::base::TCFType;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;
use gleam::gl;
use std::mem;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;

use crate::platform::{DefaultSurfaceSwapResult, Surface};
use crate::GLVersion;

lazy_static! {
    static ref CHOOSE_PIXEL_FORMAT_MUTEX: Mutex<()> = Mutex::new(());
}

// CGL OpenGL Profile that chooses a Legacy/Pre-OpenGL 3.0 Implementation.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_Legacy: CGLPixelFormatAttribute = 0x1000;
// CGL OpenGL Profile that chooses a Legacy/Pre-OpenGL 3.0 Implementation.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_3_2_Core: CGLPixelFormatAttribute = 0x3200;

pub struct Context {
    cgl_context: CGLContextObj,
}

impl Context {
    pub fn new(pixel_format: &CGLPixelFormatObj) -> Result<Context, &'static str> {
        let mut cgl_context: CGLContextObj = ptr::null_mut();
        unsafe {
            if CGLCreateContext(*pixel_format, ptr::null_mut(), &mut cgl_context) != 0 {
                return Err("CGLCreateContext failed!");
            }
        }

        debug_assert_ne!(native, ptr::null_mut());
        Ok(Context { cgl_context })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _ = self.unbind();
        if !self.weak {
            unsafe {
                if CGLDestroyContext(self.native_context) != 0 {
                    debug!("CGLDestroyContext returned an error");
                }
            }
        }
    }
}

impl Context {
    fn get_proc_address(addr: &str) -> *const () {
        let symbol_name: CFString = FromStr::from_str(addr).unwrap();
        let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
        let framework = unsafe {
            CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef())
        };
        let symbol = unsafe {
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef())
        };
        symbol as *const ()
    }

    fn current() -> Option<Self> {
        if let Some(handle) = Self::current_handle() {
            Some(Context {
                native_context: handle.0,
                weak: true,
            })
        } else {
            None
        }

    }

    fn current_handle() -> Option<Self::Handle> {
        let current = unsafe { CGLGetCurrentContext() };
        if current != 0 as CGLContextObj {
            Some(Context(current))
        } else {
            None
        }
    }

    fn create_shared(with: Option<&Self::Handle>,
                     api_type: &gl::GlType,
                     api_version: GLVersion) -> Result<Self, &'static str> {
        match *api_type {
            gl::GlType::Gles => {
                return Err("OpenGL ES is not supported");
            },
            _ => {}
        }

        // CGLChoosePixelFormat fails if multiple threads try to open a display connection
        // simultaneously. The following error is returned by CGLChoosePixelFormat: 
        // kCGLBadConnection - Invalid connection to Core Graphics.
        // We use a static mutex guard to fix this issue
        let _guard = CHOOSE_PIXEL_FORMAT_MUTEX.lock().unwrap();

        let profile = if api_version.major_version() >= 3 {
            kCGLOGLPVersion_3_2_Core
        } else {
            kCGLOGLPVersion_Legacy
        };

        let mut attributes = [
            kCGLPFAOpenGLProfile, profile,
            0
        ];

        let mut pixel_format: CGLPixelFormatObj = unsafe { mem::uninitialized() };
        let mut pix_count = 0;

        unsafe {
            if CGLChoosePixelFormat(attributes.as_mut_ptr(), &mut pixel_format, &mut pix_count) != 0 {
                return Err("CGLChoosePixelFormat");
            }

            if pix_count == 0 {
                return Err("No pixel formats available");
            }
        }

        let result = Context::new(with.map(|handle| &handle.0), &pixel_format);

        unsafe {
            if CGLDestroyPixelFormat(pixel_format) != 0 {
                debug!("CGLDestroyPixelformat errored");
            }
        }

        result
    }

    fn handle(&self) -> Self::Handle {
        Context(self.native_context)
    }

    #[inline(always)]
    fn is_current(&self) -> bool {
        unsafe {
            CGLGetCurrentContext() == self.native_context
        }
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if !self.is_current() &&
               CGLSetCurrentContext(self.native_context) != 0 {
                    Err("CGLSetCurrentContext")
            } else {
                Ok(())
            }
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        unsafe {
            if self.is_current() &&
               CGLSetCurrentContext(0 as CGLContextObj) != 0 {
                Err("CGLSetCurrentContext (on unbind)")
            } else {
                Ok(())
            }
        }
    }

    fn swap_default_surface(&mut self, new_surface: Surface) -> DefaultSurfaceSwapResult {
        DefaultSurfaceSwapResult::NotSupported { new_surface }
    }

    #[inline]
    fn uses_default_framebuffer(&self) -> bool {
        false
    }
}
