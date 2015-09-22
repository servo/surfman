extern crate glutin;

use platform::NativeGLContextMethods;
#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeDisplay;

#[cfg(not(feature="texture_surface"))]
struct NativeDisplay;

pub struct NativeGLContext {
    context: glutin::HeadlessContext,
    display: NativeDisplay,
}

impl NativeGLContextMethods for NativeGLContext {
    fn get_proc_address(addr: &str) -> *const () {
        unsafe {
            0 as *const ()
        }
    }

    fn create_headless() -> Result<Self, &'static str> {
        let display = NativeDisplay;
        return Ok(NativeGLContext {
            context: glutin::HeadlessRendererBuilder::new(128, 128).build().unwrap(),
            display: display,
        });
    }

    fn is_current(&self) -> bool {
        return self.context.is_current();
    }

    fn make_current(&self) -> Result<(), &'static str> {
        unsafe {
            match self.context.make_current() {
                Ok(()) => Ok(()),
                Err(_) => Err("MakeCurrent failed")
            }
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
