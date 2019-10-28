//! Wrapper for EGL contexts managed by ANGLE using Direct3D 11 as a backend on Windows.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::egl::types::{EGLAttrib, EGLConfig, EGLContext, EGLDeviceEXT, EGLDisplay, EGLSurface};
use crate::egl::types::{EGLenum, EGLint};
use crate::egl;
use crate::gl::types::GLuint;
use crate::gl::{self, Gl};
use crate::platform::generic::egl::context::{self, CurrentContextGuard, NativeContext};
use crate::platform::generic::egl::context::{OwnedEGLContext, UnsafeEGLContextRef};
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::platform::generic::egl::ffi::{EGL_D3D11_DEVICE_ANGLE, EGL_EXTENSION_FUNCTIONS};
use crate::platform::generic::egl::ffi::{EGL_FUNCTIONS, EGL_NO_DEVICE_EXT};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLVersion, SurfaceAccess};
use crate::{SurfaceID, SurfaceType};
use super::adapter::Adapter;
use super::device::{Device, OwnedEGLDisplay};
use super::surface::{NativeWidget, Surface, SurfaceTexture, Win32Objects};

use euclid::default::Size2D;
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;
use winapi::shared::winerror::S_OK;
use winapi::um::d3d11::ID3D11Device;
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_UNKNOWN;
use winapi::um::winbase::INFINITE;
use wio::com::ComPtr;

pub use crate::platform::generic::egl::context::ContextDescriptor;

const EGL_DEVICE_EXT: EGLenum = 0x322c;

thread_local! {
    pub static GL_FUNCTIONS: Gl = Gl::load_with(context::get_proc_address);
}

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<Surface>,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.native_context.is_destroyed() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

impl Device {
    #[inline]
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        unsafe {
            ContextDescriptor::new(self.native_display.egl_display(), config_attributes, &[
                egl::BIND_TO_TEXTURE_RGBA as EGLint,    1 as EGLint,
                egl::SURFACE_TYPE as EGLint,            egl::PBUFFER_BIT as EGLint,
                egl::RENDERABLE_TYPE as EGLint,         egl::OPENGL_ES2_BIT as EGLint,
            ])
        }
    }

    /// Opens the device and context corresponding to the current EGL context.
    ///
    /// The native context is not retained, as there is no way to do this in the EGL API. It is
    /// the caller's responsibility to keep it alive for the duration of this context. Be careful
    /// when using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library; for example, by Glutin. It's legal to use this method to wrap a context rendering
    /// to any target: either a window or a pbuffer. The target is opaque to `surfman`; the
    /// library will not modify or try to detect the render target. This means that any of the
    /// methods that query or replace the surface—e.g. `replace_context_surface`—will fail if
    /// called with a context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Grab the current EGL display and EGL context.
        let egl_display = EGL_FUNCTIONS.GetCurrentDisplay();
        if egl_display == egl::NO_DISPLAY {
            return Err(Error::NoCurrentContext);
        }
        let egl_context = EGL_FUNCTIONS.GetCurrentContext();
        if egl_context == egl::NO_CONTEXT {
            return Err(Error::NoCurrentContext);
        }
        let native_context = Box::new(UnsafeEGLContextRef { egl_context });

        // Fetch the EGL device.
        let mut egl_device = EGL_NO_DEVICE_EXT;
        let eglQueryDisplayAttribEXT =
            EGL_EXTENSION_FUNCTIONS.QueryDisplayAttribEXT
                                   .expect("Where's the `EGL_EXT_device_query` extension?");
        let result = eglQueryDisplayAttribEXT(
            egl_display,
            EGL_DEVICE_EXT as EGLint,
            &mut egl_device as *mut EGLDeviceEXT as *mut EGLAttrib);
        assert_ne!(result, egl::FALSE);
        debug_assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

        // Fetch the D3D11 device.
        let mut d3d11_device: *mut ID3D11Device = ptr::null_mut();
        let eglQueryDeviceAttribEXT =
            EGL_EXTENSION_FUNCTIONS.QueryDeviceAttribEXT
                                   .expect("Where's the `EGL_EXT_device_query` extension?");
        let result = eglQueryDeviceAttribEXT(
            egl_device,
            EGL_D3D11_DEVICE_ANGLE,
            &mut d3d11_device as *mut *mut ID3D11Device as *mut EGLAttrib);
        assert_ne!(result, egl::FALSE);
        assert!(!d3d11_device.is_null());
        let d3d11_device = ComPtr::from_raw(d3d11_device);

        // Create the device wrapper.
        // FIXME(pcwalton): Using `D3D_DRIVER_TYPE_UNKNOWN` is unfortunate. Perhaps we should
        // detect the "Microsoft Basic" string and switch to `D3D_DRIVER_TYPE_WARP` as appropriate.
        let device = Device {
            native_display: Box::new(OwnedEGLDisplay { egl_display }),
            egl_device,
            d3d11_device,
            d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
        };

        // Create the context.
        let mut context = Context {
            native_context,
            id: *next_context_id,
            framebuffer: Framebuffer::External,
        };
        next_context_id.0 += 1;

        Ok((device, context))
    }

    pub fn create_context(&mut self,
                          descriptor: &ContextDescriptor,
                          surface_access: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        unsafe {
            let egl_context = context::create_context(self.native_display.egl_display(),
                                                      descriptor)?;

            let mut context = Context {
                native_context: Box::new(OwnedEGLContext { egl_context }),
                id: *next_context_id,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;

            let initial_surface = self.create_surface(&context, surface_access, surface_type)?;
            self.attach_surface(&mut context, initial_surface);

            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Some(surface) = self.release_surface(context) {
            self.destroy_surface(context, surface);
        }

        unsafe {
            context.native_context.destroy(self);
        }

        Ok(())
    }

    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            ContextDescriptor::from_egl_context(self.native_display.egl_display(),
                                                context.native_context.egl_context())
        }
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let (egl_surface, size) = match context.framebuffer {
                Framebuffer::Surface(ref surface) => (surface.egl_surface, surface.size),
                Framebuffer::None | Framebuffer::External => {
                    return Err(Error::ExternalRenderTarget)
                }
            };
            let result = EGL_FUNCTIONS.MakeCurrent(self.native_display.egl_display(),
                                                   egl_surface,
                                                   egl_surface,
                                                   context.native_context.egl_context());
            if result == egl::FALSE {
                let err = EGL_FUNCTIONS.GetError().to_windowing_api_error();
                return Err(Error::MakeCurrentFailed(err));
            }

            Ok(())
        }
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            context::make_no_context_current(self.native_display.egl_display()
        }
    }

    fn temporarily_make_context_current(&self, context: &Context)
                                        -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    pub(crate) fn context_is_current(&self, context: &Context) -> bool {
        unsafe {
            EGL_FUNCTIONS.GetCurrentContext() == context.native_context.egl_context()
        }
    }

    fn context_surface<'c>(&self, context: &'c Context) -> Result<&'c Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(surface),
        }
    }

    fn context_surface_mut<'c>(&self, context: &'c mut Context)
                               -> Result<&'c mut Surface, Error> {
        match context.framebuffer {
            Framebuffer::None => unreachable!(),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref mut surface) => Ok(surface),
        }
    }

    pub fn replace_context_surface(&self, context: &mut Context, new_surface: Surface)
                                   -> Result<Surface, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        if context.id != new_surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        unsafe {
            let is_current = self.context_is_current(context);

            // If the surface does not use a DXGI keyed mutex, then finish.
            // FIXME(pcwalton): Is this necessary and sufficient?
            if !new_surface.uses_keyed_mutex() {
                let guard = self.temporarily_make_context_current(context)?;
                GL_FUNCTIONS.with(|gl| gl.Finish());
            }

            let old_surface = self.release_surface(context).expect("Where's our surface?");
            self.attach_surface(context, new_surface);

            if is_current {
                // We need to make ourselves current again, because the surface changed.
                self.make_context_current(context)?;
            }

            Ok(old_surface)
        }
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        Ok(0)
    }

    #[inline]
    pub fn context_surface_size(&self, context: &Context) -> Result<Size2D<i32>, Error> {
        self.context_surface(context).map(|surface| surface.size())
    }

    #[inline]
    pub fn context_surface_id(&self, context: &Context) -> Result<SurfaceID, Error> {
        self.context_surface(context).map(|surface| surface.id())
    }

    #[inline]
    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        unsafe {
            context_descriptor.attributes(self.native_display.egl_display())
        }
    }

    #[inline]
    pub fn present_context_surface(&self, context: &mut Context) -> Result<(), Error> {
        self.context_surface_mut(context).and_then(|surface| {
            self.present_surface_without_context(surface)
        })
    }

    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        context::get_proc_address(symbol_name)
    }

    #[inline]
    pub(crate) fn context_descriptor_to_egl_config(&self, context_descriptor: &ContextDescriptor)
                                                   -> EGLConfig {
        unsafe {
            context::egl_config_from_id(self.native_display.egl_display(),
                                        context_descriptor.egl_config_id)
        }
    }

    fn attach_surface(&self, context: &mut Context, surface: Surface) {
        match context.framebuffer {
            Framebuffer::None => {}
            _ => panic!("Tried to attach a surface, but there was already a surface present!"),
        }

        match surface.win32_objects {
            Win32Objects::Pbuffer { keyed_mutex: Some(ref keyed_mutex), .. } => {
                unsafe {
                    let result = keyed_mutex.AcquireSync(0, INFINITE);
                    assert_eq!(result, S_OK);
                }
            }
            _ => {}
        }

        context.framebuffer = Framebuffer::Surface(surface);
    }

    fn release_surface(&self, context: &mut Context) -> Option<Surface> {
        let surface = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => surface,
            Framebuffer::None | Framebuffer::External => return None,
        };

        match surface.win32_objects {
            Win32Objects::Pbuffer { keyed_mutex: Some(ref keyed_mutex), .. } => {
                unsafe {
                    let result = keyed_mutex.ReleaseSync(0);
                    assert_eq!(result, S_OK);
                }
            }
            _ => {}
        }

        Some(surface)
    }
}
