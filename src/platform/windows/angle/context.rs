//! Wrapper for EGL contexts managed by ANGLE using Direct3D 11 as a backend on Windows.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLFlavor, GLInfo};
use crate::{GLVersion, ReleaseContext};
use super::adapter::Adapter;
use super::device::Device;
use super::error::ToWindowingApiError;
use super::surface::{ColorSurface, Surface, SurfaceTexture};
use cgl::{CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDescribePixelFormat};
use cgl::{CGLDestroyContext, CGLError, CGLGetCurrentContext, CGLGetPixelFormat};
use cgl::{CGLPixelFormatAttribute, CGLPixelFormatObj, CGLSetCurrentContext, kCGLPFAAlphaSize};
use cgl::{kCGLPFADepthSize, kCGLPFAStencilSize, kCGLPFAOpenGLProfile};
use core_foundation::base::TCFType;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;
use gl;
use gl::types::GLuint;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;

lazy_static! {
    static ref CREATE_CONTEXT_MUTEX: Mutex<bool> = Mutex::new(false);
}

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    gl_info: GLInfo,
    color_surface: Option<ColorSurface>,
}

pub(crate) trait NativeContext {
    fn egl_context(&self) -> EGLContext;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
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
    /// Opens the device and context corresponding to the current EGL context.
    ///
    /// The native context is not retained, as there is no way to do this in the EGL API. It is the
    /// caller's responsibility to keep it alive for the duration of this context. Be careful when
    /// using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library; for example, by Glutin. It's legal to use this method to wrap a context rendering
    /// to any target: either a window or a pbuffer. The target is opaque to `surfman`; the library
    /// will not modify or try to detect the render target. This means that any of the methods that
    /// query or replace the surface—e.g. `replace_context_color_surface`—will fail if called with
    /// a context object created via this method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        let mut previous_context_created = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Grab the current EGL display and EGL context.
        let egl_display = egl::GetCurrentDisplay();
        debug_assert_ne!(egl_display, egl::NO_DISPLAY);
        let egl_context = egl::GetCurrentContext();
        debug_assert_ne!(egl_context, egl::NO_CONTEXT);
        let native_context = Box::new(UnsafeEGLContextRef { egl_context });

        println!("Device::from_current_context() = {:x}", egl_context);

        // Fetch the EGL device.
        let mut egl_device = EGL_NO_DEVICE_EXT;
        let result = eglQueryDisplayAttribEXT(egl_display, EGL_DEVICE_EXT, &mut egl_device);
        assert_ne!(result, egl::FALSE);
        debug_assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

        // Fetch the D3D11 device.
        let mut d3d11_device = ptr::null_mut();
        let result = eglQueryDeviceAttribEXT(egl_device,
                                             EGL_D3D11_DEVICE_ANGLE,
                                             &mut d3d11_device);
        assert_ne!(result, egl::FALSE);
        assert!(!d3d11_device.is_null());

        // Create the device wrapper.
        // FIXME(pcwalton): Using `D3D_DRIVER_TYPE_UNKNOWN` is unfortunate. Perhaps we should
        // detect the "Microsoft Basic" string and switch to `D3D_DRIVER_TYPE_WARP` as appropriate.
        let device = Device {
            egl_device,
            egl_display,
            surface_bindings: vec![],
            d3d11_device,
            d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
        };

        // Detect the GL version.
        let mut client_version = 0;
        let result = egl::QueryContext(egl_display,
                                       egl_context,
                                       egl::CONTEXT_CLIENT_VERSION,
                                       &mut client_version);
        assert_ne!(result, egl::FALSE);
        assert!(client_version > 0);
        let version = GLVersion::new(client_version, 0):
        println!("client version = {}", client_version);

        // Detect the config ID.
        let mut egl_config_id = 0;
        let result = egl::QueryContext(egl_display,
                                       egl_context,
                                       egl::CONFIG_ID,
                                       &mut egl_config_id);
        assert_ne!(result, egl::FALSE);

        // Fetch the current config.
        let (mut egl_config, mut egl_config_count) = (0, 0);
        let egl_config_attrs = [
            egl::CONFIG_ID as EGLint, egl_config_id,
            egl::NONE as EGLint, egl::NONE as EGLint,
            0, 0,
        ];
        let result = egl::ChooseConfig(egl_display,
                                       &egl_config_attrs[0],
                                       &mut egl_config,
                                       1,
                                       &mut egl_config_count);
        assert_ne!(result, egl::FALSE);
        assert!(egl_config_count > 0);

        // Detect pixel format.
        let alpha_size = get_config_attr(egl_display, egl_config, egl::ALPHA_SIZE);
        let depth_size = get_config_attr(egl_display, egl_config, egl::DEPTH_SIZE);
        let stencil_size = get_config_attr(egl_display, egl_config, egl::STENCIL_SIZE);

        // Convert to `surfman` context attribute flags.
        let mut attribute_flags = ContextAttributeFlags::empty();
        attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
        attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
        attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

        // Create appropriate context attributes.
        let attributes = ContextAttributes {
            flags: attribute_flags,
            flavor: GLFlavor { api: GLApi::GL, version },
        };

        let mut context = Context {
            native_context,
            gl_info: GLInfo::new(&attributes),
            color_surface: ColorSurface::External,
        };

        if !*previous_context_created {
            gl::load_with(|symbol| {
                device.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
            });
            *previous_context_created = true;
        }

        context.gl_info.populate();
        return Ok((device, context));

        unsafe fn get_config_attr(display: EGLDisplay, config: EGLConfig, attr: EGLint) -> EGLint {
            let mut value = 0;
            let result = egl::GetConfigAttrib(display, config, attr, &mut value);
            debug_assert_ne!(result, egl::FALSE);
            value
        }
    }

    pub fn create_context(&self, attributes: &ContextAttributes) -> Result<Context, Error> {
        if attributes.flavor.api == GLApi::GLES {
            return Err(Error::UnsupportedGLType);
        }

        let mut previous_context_created = CREATE_CONTEXT_MUTEX.lock().unwrap();

        let renderable_type = match attributes.flavor.api {
            GLApi::GL => egl::OPENGL_BIT,
            GLApi::GLES => egl::OPENGL_ES2_BIT,
        };

        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        unsafe {
            // Create config attributes.
            let config_attributes = [
                egl::SURFACE_TYPE as EGLint,         egl::PBUFFER_BIT as EGLint,
                egl::RENDERABLE_TYPE as EGLint,      renderable_type as EGLint,
                egl::BIND_TO_TEXTURE_RGBA as EGLint, 1 as EGLint,
                egl::RED_SIZE as EGLint,             8,
                egl::GREEN_SIZE as EGLint,           8,
                egl::BLUE_SIZE as EGLint,            8,
                egl::ALPHA_SIZE as EGLint,           alpha_size,
                egl::DEPTH_SIZE as EGLint,           depth_size,
                egl::STENCIL_SIZE as EGLint,         stencil_size,
                egl::NONE as EGLint,                 0,
                0,                                   0,
            ];

            // Pick a config.
            let (mut config, mut config_count) = (ptr::null_mut(), 0);
            let result = egl::ChooseConfig(self.native_display.egl_display(),
                                           config_attributes.as_ptr(),
                                           &mut config,
                                           1,
                                           config_count);
            if result == egl::FALSE || config_count == 0 || config.is_null() {
                return Err(Error::NoPixelFormatFound);
            }

            // Include some extra zeroes to work around broken implementations.
            let attributes = [
                egl::CONTEXT_CLIENT_VERSION as EGLint, attributes.flavor.version.major,
                egl::NONE as EGLint, 0,
                0, 0,
            ];

            let mut egl_context = egl::CreateContext(self.native_display.egl_display(),
                                                     config,
                                                     egl::NO_CONTEXT,
                                                     attributes.as_ptr());
            if egl_context == egl::NO_CONTEXT {
                return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
            }
            let native_context = OwnedEGLContext { egl_context };

            // FIXME(pcwalton): This might not work on all EGL implementations. We might have to
            // make a dummy surface.
            let result = egl::MakeCurrent(self.native_display.egl_display(),
                                          egl::NO_SURFACE,
                                          egl::NO_SURFACE,
                                          native_context.egl_context());
            if result == egl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }

            let mut context = Context {
                cgl_context,
                framebuffer: Framebuffer::None,
                gl_info: GLInfo::new(attributes),
            };

            if !*previous_context_created {
                gl::load_with(|symbol| {
                    self.get_proc_address(&mut context, symbol).unwrap_or(ptr::null())
                });
                *previous_context_created = true;
            }

            context.gl_info.populate();
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Some(color_surface) = context.color_surface.take() {
            self.destroy_surface(context, color_surface);
        }

        context.native_context.destroy(context);
        Ok(())
    }

    #[inline]
    pub fn context_gl_info<'c>(&self, context: &'c Context) -> &'c GLInfo {
        &context.gl_info
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let color_egl_surface = match context.color_surface {
                Some(ref color_surface) => self.lookup_surface(color_surface),
                None => egl::NO_SURFACE,
            };
            let result = egl::MakeCurrent(self.native_display.egl_display(),
                                          color_egl_surface,
                                          color_egl_surface,
                                          context.native_context.egl_context());
            if result == egl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Error));
            }
            Ok(())
        }
    }

    pub fn make_context_not_current(&self, _: &Context) -> Result<(), Error> {
        unsafe {
            let result = egl::MakeCurrent(self.native_display.egl_display(),
                                          egl::NO_SURFACE,
                                          egl::NO_SURFACE,
                                          egl::NO_CONTEXT);
            if result == egl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Error));
            }
            Ok(())
        }
    }

    pub fn get_proc_address(&self, _: &Context, symbol_name: &str)
                            -> Result<*const c_void, Error> {
        unsafe {
            let framework_identifier: CFString =
                FromStr::from_str(OPENGL_FRAMEWORK_IDENTIFIER).unwrap();
            let framework =
                CFBundleGetBundleWithIdentifier(framework_identifier.as_concrete_TypeRef());
            if framework.is_null() {
                return Err(Error::NoGLLibraryFound);
            }

            let symbol_name: CFString = FromStr::from_str(symbol_name).unwrap();
            let fun_ptr = CFBundleGetFunctionPointerForName(framework,
                                                            symbol_name.as_concrete_TypeRef());
            if fun_ptr.is_null() {
                return Err(Error::GLFunctionNotFound);
            }
            
            return Ok(fun_ptr as *const c_void);
        }

        static OPENGL_FRAMEWORK_IDENTIFIER: &'static str = "com.apple.opengl";
    }

    #[inline]
    pub fn context_color_surface<'c>(&self, context: &'c Context) -> Option<&'c Surface> {
        match context.framebuffer {
            Framebuffer::None | Framebuffer::External => None,
            Framebuffer::Object { ref color_surface_texture, .. } => {
                Some(&color_surface_texture.surface)
            }
        }
    }

    pub fn replace_context_color_surface(&self, context: &mut Context, new_color_surface: Surface)
                                         -> Result<Option<Surface>, Error> {
        if let Framebuffer::External = context.framebuffer {
            return Err(Error::ExternalRenderTarget)
        }

        self.make_context_current(context)?;

        // Make sure all changes are synchronized. Apple requires this.
        unsafe {
            gl::Flush();
        }

        // Fast path: we have a FBO set up already and the sizes are the same. In this case, we can
        // just switch the backing texture.
        let can_modify_existing_framebuffer = match context.framebuffer {
            Framebuffer::Object { ref color_surface_texture, .. } => {
                // FIXME(pcwalton): Should we check parts of the descriptor other than size as
                // well?
                color_surface_texture.surface().descriptor().size ==
                    new_color_surface.descriptor().size
            }
            Framebuffer::None | Framebuffer::External => false,
        };
        if can_modify_existing_framebuffer {
            return self.replace_color_surface_in_existing_framebuffer(context, new_color_surface)
                       .map(Some);
        }

        let (old_surface, result) = self.destroy_framebuffer(context);
        if let Err(err) = result {
            if let Some(old_surface) = old_surface {
                drop(self.destroy_surface(context, old_surface));
            }
            return Err(err);
        }
        if let Err(err) = self.create_framebuffer(context, new_color_surface) {
            if let Some(old_surface) = old_surface {
                drop(self.destroy_surface(context, old_surface));
            }
            return Err(err);
        }

        Ok(old_surface)
    }

    #[inline]
    pub fn context_surface_framebuffer_object(&self, context: &Context) -> Result<GLuint, Error> {
        match context.framebuffer {
            Framebuffer::None => Err(Error::NoSurfaceAttached),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Object { framebuffer_object, .. } => Ok(framebuffer_object),
        }
    }

    // Assumes that the context is current.
    fn create_framebuffer(&self, context: &mut Context, color_surface: Surface)
                          -> Result<(), Error> {
        let descriptor = *color_surface.descriptor();
        let color_surface_texture = self.create_surface_texture(context, color_surface)?;

        unsafe {
            let mut framebuffer_object = 0;
            gl::GenFramebuffers(1, &mut framebuffer_object);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     SurfaceTexture::gl_texture_target(),
                                     color_surface_texture.gl_texture(),
                                     0);

            let renderbuffers = Renderbuffers::new(&descriptor.size, &context.gl_info);
            renderbuffers.bind_to_current_framebuffer();

            debug_assert_eq!(gl::CheckFramebufferStatus(gl::FRAMEBUFFER),
                             gl::FRAMEBUFFER_COMPLETE);

            // Set the viewport so that the application doesn't have to do so explicitly.
            gl::Viewport(0, 0, descriptor.size.width, descriptor.size.height);

            context.framebuffer = Framebuffer::Object {
                framebuffer_object,
                color_surface_texture,
                renderbuffers,
            };
        }

        Ok(())
    }

    fn destroy_framebuffer(&self, context: &mut Context) -> (Option<Surface>, Result<(), Error>) {
        let (framebuffer_object,
             color_surface_texture,
             mut renderbuffers) = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::External => unreachable!(),
            Framebuffer::None => return (None, Ok(())),
            Framebuffer::Object { framebuffer_object, color_surface_texture, renderbuffers } => {
                (framebuffer_object, color_surface_texture, renderbuffers)
            }
        };

        let old_surface = match self.destroy_surface_texture(context, color_surface_texture) {
            Ok(old_surface) => old_surface,
            Err(err) => return (None, Err(err)),
        };

        renderbuffers.destroy();

        unsafe {
            gl::DeleteFramebuffers(1, &framebuffer_object);
        }

        (Some(old_surface), Ok(()))
    }

    fn replace_color_surface_in_existing_framebuffer(&self,
                                                     context: &mut Context,
                                                     new_color_surface: Surface)
                                                     -> Result<Surface, Error> {
        println!("replace_color_surface_in_existing_framebuffer()");
        let new_color_surface_texture = self.create_surface_texture(context, new_color_surface)?;

        let (framebuffer_object, framebuffer_color_surface_texture) = match context.framebuffer {
            Framebuffer::Object { framebuffer_object, ref mut color_surface_texture, .. } => {
                (framebuffer_object, color_surface_texture)
            }
            _ => unreachable!(),
        };

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     SurfaceTexture::gl_texture_target(),
                                     new_color_surface_texture.gl_texture(),
                                     0);
        }

        let old_color_surface_texture = mem::replace(framebuffer_color_surface_texture,
                                                     new_color_surface_texture);
        self.destroy_surface_texture(context, old_color_surface_texture)
    }
}

struct OwnedEGLContext {
    egl_context: EGLContext,
}

impl ReleaseContext for OwnedEGLContext {
    #[inline]
    fn egl_context(&self) -> EGLContext {
        self.egl_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_context == egl::NO_CONTEXT
    }

    unsafe fn destroy(&mut self, device: &Device) {
        assert!(!self.is_destroyed());
        egl::MakeCurrent(device.native_display.egl_display(),
                         egl::NO_SURFACE,
                         egl::NO_SURFACE,
                         egl::NO_CONTEXT);
        let result = egl::DestroyContext(device.native_display.egl_display(), self.egl_context);
        assert_ne!(result, egl::FALSE);
        self.egl_context = egl::NO_CONTEXT;
    }
}

struct UnsafeEGLContextRef {
    egl_context: EGLContext,
}

impl ReleaseContext for UnsafeEGLContextRef {
    #[inline]
    fn egl_context(&self) -> EGLContext {
        self.egl_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_context == egl::NO_CONTEXT
    }

    unsafe fn destroy(&mut self, device: &Device) {
        assert!(!self.is_destroyed());
        self.egl_context = egl::NO_CONTEXT;
    }
}
