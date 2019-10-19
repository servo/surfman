use crate::platform::NativeGLContextMethods;
use crate::GLVersion;
use objc::runtime::{BOOL, NO};
use objc::runtime::{Class, Object};
use libloading as lib;
use sparkle::gl;
use std::ops::Deref;
use std::ptr;

type EAGLContext = *mut Object;
type EAGLSharegroup = *mut Object;

lazy_static! {
    static ref EAGLCONTEXT_CLASS: &'static Class  = {
        Class::get("EAGLContext").expect("EAGLContext class not found")
    };

    static ref OPENGLES_FRAMEWORK: Option<lib::Library>  = {
        lib::Library::new("/System/Library/Frameworks/OpenGLES.framework/OpenGLES").ok()
    };
}

pub struct NativeGLContext(EAGLContext);
pub type NativeGLContextHandle = NativeGLContext;
unsafe impl Send for NativeGLContext {}

impl NativeGLContext {
    fn new_retained(context: EAGLContext) -> Self {
        let context = unsafe { msg_send![context, retain] };
        NativeGLContext(context)
    }
}

impl Drop for NativeGLContext {
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.0, release];
        }
    }
}

impl NativeGLContextMethods for NativeGLContext {
    type Handle = Self;

    fn get_proc_address(addr: &str) -> *const () {
        let framework = match *OPENGLES_FRAMEWORK {
            Some(ref lib) => lib,
            None => return ptr::null(),
        };
        unsafe {
            let symbol: Result<lib::Symbol<unsafe extern fn()>, _> = framework.get(addr.as_bytes());
            match symbol {
                Ok(symbol) => *symbol.deref() as *const (),
                _ => ptr::null()
            }
        }
    }

    fn create_shared(with: Option<&Self::Handle>,
                     api_type: &gl::GlType,
                     api_version: GLVersion) -> Result<NativeGLContext, &'static str> {
        match *api_type {
            gl::GlType::Gles => {},
            _ => return Err("Only OpenGL ES is supported on iOS"),
        }
        let context: EAGLContext = unsafe {
            let context: EAGLContext = msg_send![*EAGLCONTEXT_CLASS, alloc];
            let version = api_version.major_version() as u32;
            match with {
                Some(with) => {
                    let sharegroup: EAGLSharegroup = msg_send![with.0, sharegroup];
                    msg_send![context, initWithAPI: version sharegroup: sharegroup]
                },
                None => msg_send![context, initWithAPI: version]
            }
        };

        if context.is_null() {
            Err("[EAGLContext initWithAPI] failed")
        } else {
            // Instance already retained by the alloc call. No need to call NativeGLContext::new_retained.
            Ok(NativeGLContext(context))
        }
    }

    fn is_current(&self) -> bool {
        match Self::current_handle() {
            Some(handle) => handle.0 == self.0,
            None => false
        }
    }

    fn current() -> Option<Self> {
        let context: EAGLContext = unsafe {
            msg_send![*EAGLCONTEXT_CLASS, currentContext]
        };
        if context.is_null() {
            None
        } else {
            Some(NativeGLContext::new_retained(context))
        }
    }

    fn current_handle() -> Option<Self::Handle> {
        let context: EAGLContext = unsafe {
            msg_send![*EAGLCONTEXT_CLASS, currentContext]
        };
        if context.is_null() {
            None
        } else {
            Some(NativeGLContext::new_retained(context))
        }
    }

    fn make_current(&self) -> Result<(), &'static str> {
        let succeeded: BOOL = unsafe {
            msg_send![*EAGLCONTEXT_CLASS, setCurrentContext: self.0]
        };
        if succeeded == NO {
            Err("[EAGLContext setCurrentContext: context] failed")
        } else {
            Ok(())
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        let succeeded: BOOL = unsafe {
            msg_send![*EAGLCONTEXT_CLASS, setCurrentContext: 0 as EAGLContext]
        };
        if succeeded == NO {
            Err("[EAGLContext setCurrentContext: nil] failed")
        } else {
            Ok(())
        }
    }

    fn handle(&self) -> Self::Handle {
        NativeGLContext::new_retained(self.0)
    }
}
