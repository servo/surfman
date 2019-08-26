use std::ffi::CString;
use std::os::raw::c_int;
use std::ptr;

use crate::gl_context::GLVersion;
use crate::platform::{DefaultSurfaceSwapResult, NativeSurface};
use gleam::gl;

const DUMMY_BUFFER_WIDTH: usize = 16;
const DUMMY_BUFFER_HEIGHT: usize = 16;

pub struct OSMesaContext {
    buffer: Vec<u8>,
    context: osmesa_sys::OSMesaContext,
}

pub struct OSMesaContextHandle(osmesa_sys::OSMesaContext);

unsafe impl Send for OSMesaContextHandle {}

impl OSMesaContext {
    pub fn new(share_with: Option<osmesa_sys::OSMesaContext>,
               api_type: &gl::GlType,
               api_version: GLVersion)
        -> Result<Self, &'static str> {
        let shared = match share_with {
            Some(ctx) => ctx,
            _ => ptr::null_mut(),
        };

        match *api_type {
            gl::GlType::Gles => {
                return Err("OpenGL ES is not supported");
            },
            _ => {}
        }

        let (major, minor) = match api_version {
            // OSMesa only supports compatibility (non-Core) profiles in GL versions <= 3.0.
            // A 3.0 compatibility profile is preferred for a major 3 context version (e.g. WebGL 2).
            // A 2.1 profile is created for a major 2 context version (e.g. WebGL 1).
            GLVersion::Major(major) => (major, if major >= 3 { 0 } else { 1 }),
            GLVersion::MajorMinor(major, minor) => (major, minor)
        };

        let attributes = [
            osmesa_sys::OSMESA_FORMAT, osmesa_sys::OSMESA_RGBA as c_int,
            osmesa_sys::OSMESA_CONTEXT_MAJOR_VERSION, major as c_int,
            osmesa_sys::OSMESA_CONTEXT_MINOR_VERSION, minor as c_int,
            0
        ];

        let context = unsafe {
            osmesa_sys::OSMesaCreateContextAttribs(attributes.as_ptr(), shared)
        };

        if context.is_null() {
            return Err("OSMesaCreateContext");
        }

        let buffer = vec![0u8; DUMMY_BUFFER_WIDTH * DUMMY_BUFFER_HEIGHT * 4];
        Ok(OSMesaContext {
            buffer: buffer,
            context: context,
        })
    }

    fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            ::std::mem::transmute(
                osmesa_sys::OSMesaGetProcAddress(addr as *const _))
        }
    }

    fn current_handle() -> Option<Self::Handle> {
        let current = unsafe { osmesa_sys::OSMesaGetCurrentContext() };
        if current.is_null() {
            None
        } else {
            Some(OSMesaContextHandle(current))
        }
    }

    fn current() -> Option<Self> {
        /* We can't access to the OSMesa buffer from here. */
        None
    }

    fn create_shared(with: Option<&Self::Handle>,
                     api_type: &gl::GlType,
                     api_version: GLVersion) -> Result<Self, &'static str> {
        Self::new(with.map(|w| w.0), api_type, api_version)
    }

    fn is_current(&self) -> bool {
        unsafe {
            osmesa_sys::OSMesaGetCurrentContext() == self.context
        }
    }

    fn handle(&self) -> Self::Handle {
        OSMesaContextHandle(self.context)
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            if !self.is_current() &&
               osmesa_sys::OSMesaMakeCurrent(self.context,
                                             self.buffer.as_ptr() as *const _ as *mut _,
                                             gl::UNSIGNED_BYTE,
                                             DUMMY_BUFFER_WIDTH as i32,
                                             DUMMY_BUFFER_HEIGHT as i32) == 0 {
               Err("OSMesaMakeCurrent")
           } else {
               Ok(())
           }
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        if self.is_current() {
            let ret = unsafe {
                osmesa_sys::OSMesaMakeCurrent(ptr::null_mut(),
                                              ptr::null_mut(), 0, 0, 0)
            };
            if ret == gl::FALSE {
                return Err("OSMesaMakeCurrent");
            }
        }

        Ok(())
    }

    fn swap_default_surface(&mut self, new_surface: NativeSurface) -> DefaultSurfaceSwapResult {
        DefaultSurfaceSwapResult::Failed { message: "TODO", new_surface }
    }

    #[inline]
    fn uses_default_framebuffer(&self) -> bool { true }

    fn is_osmesa(&self) -> bool { true }
}

impl Drop for OSMesaContext {
    fn drop(&mut self) {
        unsafe { osmesa_sys::OSMesaDestroyContext(self.context) }
    }
}
