use platform::NativeGLContextMethods;
use glutin::{HeadlessContext, HeadlessRendererBuilder};

#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeDisplay;

#[cfg(not(feature="texture_surface"))]
struct NativeDisplay;

#[cfg(not(feature="texture_surface"))]
impl NativeDisplay {
    fn new() -> NativeDisplay {
        NativeDisplay
    }
}


pub struct NativeGLContext {
    context: HeadlessContext,
    display: NativeDisplay,
}

impl NativeGLContextMethods for NativeGLContext {
    fn get_proc_address(_addr: &str) -> *const () {
        0 as *const ()
    }

    fn create_headless() -> Result<Self, &'static str> {
        let builder = HeadlessRendererBuilder::new(128, 128);
        let glutin_context = try!(builder.build().or(Err("Glutin Headless context creation error")));

        Ok(NativeGLContext {
            context: glutin_context,
            display: NativeDisplay::new(),
        })
    }

    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            self.context.make_current().or(Err("MakeCurrent failed"))
        }
    }

    fn unbind(&self) -> Result<(), &'static str> {
        Ok(())
    }

    #[cfg(feature="texture_surface")]
    fn get_display(&self) -> NativeDisplay {
        self.display
    }
}
