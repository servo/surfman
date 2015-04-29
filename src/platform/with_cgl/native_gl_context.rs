use NativeGLContextMethods;

pub struct NativeGLContext {
    native_context: CGLContextObj,
}

impl NativeGLContext {
    // NOTE: this function doesn't destroy the associated the
    //   corresponding CGLPixelFormatObj.
    //
    //   While this can be desirable, we can't rely on it.
    pub fn new(share_context: Option<NativeGLContext>,
               pixel_format: &CGLPixelFormatObj)
        -> Result<NativeGLContext, &'static str> {

        let shared = match share_context {
            Some(ctx) => ctx.as_native_cgl_context(),
            None => 0 as CGLContextObj
        };

        let native = unsafe { mem::uninitialized() };

        unsafe {
            if CGLCreateContext(pixel_format, shared, &mut native) != 0 {
                return Err("CGLCreateContext");
            }
        }

        debug_assert!(native != 0 as CGLContextObj);

        Ok(NativeGLContext {
            native_context: native,
        })
    }

    pub fn as_native_cgl_context(&self) -> CGLContextObj {
        self.native_context
    }
}

impl NativeGLContextMethods for NativeGLContext{
    fn create_headless() -> Result<NativeGLContext, &'static str> {
        // NOTE: This attributes force hw acceleration,
        //   we may want to allow non hw-accelerated contexts
        let mut attributes = [
            kCGLPFAAccelerated,
            0
        ];

        let native : CGLContextObj = unsafe { mem::uninitialized() };
        let pixel_format : CGLPixelFormatObj = unsafe { mem::uninitialized() };

        let pix_count = 0;

        unsafe {
            if CGLChoosePixelFormat(attributes.as_mut_ptr(), &mut pixel_format, &mut pix_count) != 0 {
                return Err("CGLChoosePixelFormat");
            }

            if pix_count == 0 {
                return Err("No pixel formats available");
            }
        }

        let result = NativeGLContext::new(None, &pixel_format);

        CGLDestroyPixelFormat(pixel_format);

        result
    }

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
}
