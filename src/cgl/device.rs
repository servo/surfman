//! A handle to the device. (This is a no-op, because handles are implicit in Apple's Core OpenGL.)

use super::context::{CglContext, CglContextDescriptor};
use crate::base::io_surface::device::Device as SystemDevice;
use crate::base::io_surface::surface::Surface as SystemSurface;
use crate::cgl::context::{CurrentContextGuard, NativeContext};
use crate::cgl::error::ToWindowingApiError;
use crate::cgl::ffi::{CGLReleaseContext, CGLRetainContext};
use crate::cgl::surface::{
    surface_bind_to_gl_texture, CglBinding, CglSurface, CglSurfaceTexture, CglWindowSurface,
    IoSurfaceBuffer, NativeSurface,
};
use crate::context::{ContextID, CREATE_CONTEXT_MUTEX};
use crate::renderbuffers::Renderbuffers;
use crate::surface::Framebuffer;
use crate::{
    gl, gl_utils, Adapter, Context, GLVersion, OffscreenSurface, Surface, SurfaceAccess,
    SurfaceInfo, SurfaceTexture, WindowSurface, WindowingApiError,
};
use crate::{ContextAttributeFlags, ContextAttributes, ContextDescriptor, Error, GLApi, Gl};
use cgl::{
    kCGLPFAAllowOfflineRenderers, kCGLPFAAlphaSize, kCGLPFADepthSize, kCGLPFAOpenGLProfile,
    kCGLPFAStencilSize, CGLChoosePixelFormat, CGLContextObj, CGLCreateContext,
    CGLDescribePixelFormat, CGLError, CGLGetPixelFormat, CGLPixelFormatAttribute,
    CGLRetainPixelFormat, CGLSetCurrentContext,
};
use euclid::default::Size2D;
use glow::HasContext;
use glow::Texture;
use objc2_core_foundation::{CFBundle, CFRetained, CFString};
use objc2_io_surface::IOSurfaceRef;
use raw_window_handle::WindowHandle;
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::rc::Rc;
use std::{mem, ptr};

pub use crate::base::io_surface::device::NativeDevice;

const SURFACE_GL_TEXTURE_TARGET: u32 = gl::TEXTURE_RECTANGLE;

// No CGL error occurred.
#[allow(non_upper_case_globals)]
const kCGLNoError: CGLError = 0;

// Choose a renderer compatible with GL 1.0.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_Legacy: CGLPixelFormatAttribute = 0x1000;
// Choose a renderer capable of GL3.2 or later.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_3_2_Core: CGLPixelFormatAttribute = 0x3200;
// Choose a renderer capable of GL4.1 or later.
#[allow(non_upper_case_globals)]
const kCGLOGLPVersion_GL4_Core: CGLPixelFormatAttribute = 0x4100;

thread_local! {
    static OPENGL_FRAMEWORK: CFRetained<CFBundle> = {
        static OPENGL_FRAMEWORK_IDENTIFIER: &str = "com.apple.opengl";
        let framework_identifier = CFString::from_str(OPENGL_FRAMEWORK_IDENTIFIER);
        let framework = CFBundle::bundle_with_identifier(Some(&framework_identifier));
        framework.unwrap()
    };
}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
#[derive(Clone)]
pub struct CglDevice(pub(crate) SystemDevice);

impl CglDevice {
    /// Returns the native device corresponding to this device.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        self.0.native_device()
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.0.adapter().into()
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        if attributes
            .flags
            .contains(ContextAttributeFlags::COMPATIBILITY_PROFILE)
            && attributes.version.major > 2
        {
            return Err(Error::UnsupportedGLProfile);
        };

        let profile = if attributes.version.major >= 4 {
            kCGLOGLPVersion_GL4_Core
        } else if attributes.version.major == 3 {
            kCGLOGLPVersion_3_2_Core
        } else {
            kCGLOGLPVersion_Legacy
        };

        let flags = attributes.flags;
        let alpha_size = if flags.contains(ContextAttributeFlags::ALPHA) {
            8
        } else {
            0
        };
        let depth_size = if flags.contains(ContextAttributeFlags::DEPTH) {
            24
        } else {
            0
        };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) {
            8
        } else {
            0
        };

        let mut cgl_pixel_format_attributes = vec![
            kCGLPFAOpenGLProfile,
            profile,
            kCGLPFAAlphaSize,
            alpha_size,
            kCGLPFADepthSize,
            depth_size,
            kCGLPFAStencilSize,
            stencil_size,
        ];

        // This means "opt into the integrated GPU".
        //
        // https://supermegaultragroovy.com/2016/12/10/auto-graphics-switching/
        if self.0.adapter.is_low_power {
            cgl_pixel_format_attributes.push(kCGLPFAAllowOfflineRenderers);
        }

        cgl_pixel_format_attributes.extend_from_slice(&[0, 0]);

        unsafe {
            let (mut cgl_pixel_format, mut cgl_pixel_format_count) = (ptr::null_mut(), 0);
            let err = CGLChoosePixelFormat(
                cgl_pixel_format_attributes.as_ptr(),
                &mut cgl_pixel_format,
                &mut cgl_pixel_format_count,
            );
            if err != kCGLNoError {
                return Err(Error::PixelFormatSelectionFailed(
                    err.to_windowing_api_error(),
                ));
            }
            if cgl_pixel_format_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            Ok(CglContextDescriptor { cgl_pixel_format }.into())
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        let share_with = share_with.map(Context::cgl).transpose()?;

        // Take a lock so that we're only creating one context at a time. `CGLChoosePixelFormat`
        // will fail, returning `kCGLBadConnection`, if multiple threads try to open a display
        // connection simultaneously.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        unsafe {
            // Create the CGL context.
            let mut cgl_context = ptr::null_mut();
            let err = CGLCreateContext(
                descriptor.cgl()?.cgl_pixel_format,
                share_with.map_or(ptr::null_mut(), |ctx| ctx.cgl_context),
                &mut cgl_context,
            );
            if err != kCGLNoError {
                return Err(Error::ContextCreationFailed(err.to_windowing_api_error()));
            }
            debug_assert_ne!(cgl_context, ptr::null_mut());

            if let Err(error) = make_cgl_context_current(cgl_context) {
                CGLReleaseContext(cgl_context);
                return Err(error);
            }
            // Wrap and return the context.
            let context = CglContext {
                cgl_context,
                id: *next_context_id,
                framebuffer: Framebuffer::None,
                gl: Rc::new(Gl::from_loader_function(get_proc_address)),
            };
            next_context_id.0 += 1;
            Ok(context.into())
        }
    }

    /// Wraps a `CGLContext` in a `surfman` context and returns it.
    ///
    /// This function takes ownership of the native context and does not adjust its reference
    /// count.
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext,
    ) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let context = CglContext {
            cgl_context: native_context.0,
            id: *next_context_id,
            framebuffer: Framebuffer::None,
            gl: Rc::new(Gl::from_loader_function(get_proc_address)),
        };
        next_context_id.0 += 1;
        mem::forget(native_context);
        Ok(context.into())
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        let context: &mut CglContext = context.try_into()?;
        if context.cgl_context.is_null() {
            return Ok(());
        }

        if let Framebuffer::Surface(ref mut surface) =
            mem::replace(&mut context.framebuffer, Framebuffer::None)
        {
            self.destroy_surface_inner(context, surface)?;
        }

        unsafe {
            CGLSetCurrentContext(ptr::null_mut());
            CGLReleaseContext(context.cgl_context);
            context.cgl_context = ptr::null_mut();
        }

        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        let context = context.cgl().expect("Passed incorrect context type");
        unsafe {
            let mut cgl_pixel_format = CGLGetPixelFormat(context.cgl_context);
            cgl_pixel_format = CGLRetainPixelFormat(cgl_pixel_format);
            CglContextDescriptor { cgl_pixel_format }.into()
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        let context = context.cgl()?;
        make_cgl_context_current(context.cgl_context)
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            let err = CGLSetCurrentContext(ptr::null_mut());
            if err != kCGLNoError {
                return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
            }
            Ok(())
        }
    }

    pub(crate) fn temporarily_make_context_current(
        &self,
        context: &CglContext,
    ) -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        make_cgl_context_current(context.cgl_context)?;
        Ok(guard)
    }

    /// See [`crate::Device::bind_surface_to_context`].
    pub fn bind_surface_to_context(
        &self,
        context: &mut Context,
        new_surface: Surface,
    ) -> Result<Option<Surface>, (Error, Surface)> {
        let context: &mut CglContext = match context.try_into() {
            Ok(context) => context,
            Err(error) => return Err((error, new_surface)),
        };
        let new_surface: CglSurface = new_surface.try_into()?;
        if new_surface.context_id() != context.id {
            return Err((Error::IncompatibleSurface, new_surface.into()));
        }

        let previously_bound_surface = match self.unbind_surface_from_context_inner(context) {
            Ok(surface) => surface,
            Err(error) => return Err((error, new_surface.into())),
        };

        context.framebuffer = Framebuffer::Surface(new_surface);
        Ok(previously_bound_surface)
    }

    /// See [`crate::Device::unbind_surface_from_context`].
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context,
    ) -> Result<Option<Surface>, Error> {
        self.unbind_surface_from_context_inner(context.try_into()?)
    }

    fn unbind_surface_from_context_inner(
        &self,
        context: &mut CglContext,
    ) -> Result<Option<Surface>, Error> {
        let framebuffer = {
            match context.framebuffer {
                Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
                Framebuffer::None | Framebuffer::Surface(_) => {}
            };
            mem::replace(&mut context.framebuffer, Framebuffer::None)
        };

        match framebuffer {
            Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
            Framebuffer::None => Ok(None),
            Framebuffer::Surface(surface) => {
                // Make sure all changes are synchronized. Apple requires this.
                //
                // TODO(pcwalton): Use `glClientWaitSync` instead to avoid starving the window
                // server.

                let _guard = self.temporarily_make_context_current(context)?;
                let gl = &context.gl;
                unsafe {
                    gl.flush();
                }

                if let Some(framebuffer) = surface.info().framebuffer_object {
                    gl_utils::unbind_framebuffer_if_necessary(gl, framebuffer);
                }
                Ok(Some(surface.into()))
            }
        }
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        let context: &mut CglContext = context.try_into()?;
        if let Framebuffer::Surface(surface) = &mut context.framebuffer {
            // Presenting the surface is not a GL operation on macOS, it's just
            // CoreAnimation and IOSurface management. This means that it will
            // leave any unprocessed OpenGL commands in the pipeline. Flushing
            // here ensures that doesn't happen.
            unsafe { context.gl.flush() };

            let CglSurface::Window(CglWindowSurface(binding)) = surface else {
                return Err(Error::NoWidgetAttached);
            };
            self.0.present_surface(&mut binding.system_surface)?;
            binding.bind_to_texture(&context.gl);
        }
        Ok(())
    }

    /// If the currently bound surface is a widget surface, resize it,
    pub fn resize_bound_surface(
        &self,
        context: &mut Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);

        let context: &mut CglContext = context.try_into()?;
        let _guard = self.temporarily_make_context_current(context);

        if let Framebuffer::Surface(surface) = &mut context.framebuffer {
            let CglSurface::Window(CglWindowSurface(binding)) = surface else {
                return Err(Error::NoWidgetAttached);
            };
            return self.resize_inner(binding, size, &context.gl, context_attributes);
        }
        Ok(())
    }

    /// Returns the attributes that the context descriptor was created with.
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        let context_descriptor = context_descriptor
            .cgl()
            .expect("Passed incompatible context descriptor");
        unsafe {
            let alpha_size = get_pixel_format_attribute(context_descriptor, kCGLPFAAlphaSize);
            let depth_size = get_pixel_format_attribute(context_descriptor, kCGLPFADepthSize);
            let stencil_size = get_pixel_format_attribute(context_descriptor, kCGLPFAStencilSize);
            let gl_profile = get_pixel_format_attribute(context_descriptor, kCGLPFAOpenGLProfile);

            let mut attribute_flags = ContextAttributeFlags::empty();
            attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
            attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
            attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

            let mut version = GLVersion::new(
                ((gl_profile >> 12) & 0xf) as u8,
                ((gl_profile >> 8) & 0xf) as u8,
            );
            if version.major <= 2 {
                version.major = 2;
                version.minor = 1;
                attribute_flags.insert(ContextAttributeFlags::COMPATIBILITY_PROFILE);
            }

            return ContextAttributes {
                flags: attribute_flags,
                version,
            };
        }

        unsafe fn get_pixel_format_attribute(
            context_descriptor: &CglContextDescriptor,
            attribute: CGLPixelFormatAttribute,
        ) -> i32 {
            let mut value = 0;
            let err = CGLDescribePixelFormat(
                context_descriptor.cgl_pixel_format,
                0,
                attribute,
                &mut value,
            );
            debug_assert_eq!(err, kCGLNoError);
            value
        }
    }

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        get_proc_address(symbol_name)
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        let context = context.cgl()?;
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External(_) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(surface.info())),
        }
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        let context = context.cgl().expect("Passed incorrect context type");
        context.id
    }

    /// Given a context, returns its underlying CGL context object.
    ///
    /// The reference count on that context is incremented via `CGLRetainContext()` before
    /// returning.
    #[inline]
    pub fn native_context(&self, context: &Context) -> NativeContext {
        let context = context.cgl().expect("Passed incorrect context type");
        unsafe { NativeContext(CGLRetainContext(context.cgl_context)) }
    }

    /// See [`crate::Device::create_window_surface`].
    pub fn create_window_surface(
        &self,
        context: &Context,
        access: SurfaceAccess,
        window_handle: WindowHandle<'_>,
        _: Size2D<i32>,
    ) -> Result<WindowSurface, Error> {
        let system_surface = self.0.create_window_surface(access, window_handle)?;
        let binding = self.create_cgl_binding(context, system_surface)?;
        Ok(CglWindowSurface(binding).into())
    }

    /// See [`crate::Device::create_offscreen_surface`].
    pub fn create_offscreen_surface(
        &self,
        context: &Context,
        access: SurfaceAccess,
        size: Size2D<i32>,
    ) -> Result<OffscreenSurface, Error> {
        let system_surface = self.0.create_offscreen_surface(access, size)?;
        let binding = self.create_cgl_binding(context, system_surface)?;
        Ok(IoSurfaceBuffer::from(binding).into())
    }

    fn create_cgl_binding(
        &self,
        context: &Context,
        mut system_surface: SystemSurface,
    ) -> Result<CglBinding, Error> {
        self.0.set_surface_flipped(&mut system_surface, true);

        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);

        let context = context.cgl()?;
        let _guard = self.temporarily_make_context_current(context);

        let gl = &context.gl;
        let (framebuffer_object, texture_object, renderbuffers) = unsafe {
            let texture_object =
                self.bind_to_gl_texture(gl, &system_surface.io_surface, &system_surface.size);

            let framebuffer_object = gl.create_framebuffer().unwrap();
            let _guard =
                self.temporarily_bind_framebuffer(context.gl.clone(), Some(framebuffer_object));

            gl.framebuffer_texture_2d(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                SURFACE_GL_TEXTURE_TARGET,
                Some(texture_object),
                0,
            );

            let mut renderbuffers =
                Renderbuffers::new(gl, &system_surface.size, &context_attributes);
            renderbuffers.bind_to_current_framebuffer(gl);

            if gl.get_error() != gl::NO_ERROR
                || gl.check_framebuffer_status(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE
            {
                // On macos, surface creation can fail silently (e.g. due to OOM) and AFAICT
                // the way to tell that it has failed is to look at the framebuffer status
                // while the surface is attached.
                renderbuffers.destroy(gl);
                gl.delete_framebuffer(framebuffer_object);
                gl.delete_texture(texture_object);
                let _ = self.0.destroy_surface(&mut system_surface);
                // TODO: convert the GL error into a surfman error?
                return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
            }

            (framebuffer_object, texture_object, renderbuffers)
        };

        Ok(CglBinding {
            system_surface,
            context_id: context.id,
            framebuffer_object: Some(framebuffer_object),
            texture_object: Some(texture_object),
            renderbuffers,
        })
    }

    /// Creates a surface texture from an existing generic surface for use with the given context.
    ///
    /// The surface texture is local to the supplied context and takes ownership of the surface.
    /// Destroying the surface texture allows you to retrieve the surface again.
    ///
    /// *The supplied context does not have to be the same context that the surface is associated
    /// with.* This allows you to render to a surface in one context and sample from that surface
    /// in another context.
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: OffscreenSurface,
    ) -> Result<SurfaceTexture, (Error, OffscreenSurface)> {
        let surface: IoSurfaceBuffer = surface.try_into()?;
        let context = match context.cgl() {
            Ok(context) => context,
            Err(error) => return Err((error, surface.into())),
        };

        let _guard = self.temporarily_make_context_current(context).unwrap();
        let texture_object =
            self.bind_to_gl_texture(&context.gl, &surface.io_surface, &surface.size());
        Ok(CglSurfaceTexture {
            surface,
            texture_object: Some(texture_object),
            phantom: PhantomData,
        }
        .into())
    }

    fn bind_to_gl_texture(
        &self,
        gl: &Gl,
        io_surface: &IOSurfaceRef,
        size: &Size2D<i32>,
    ) -> Texture {
        unsafe {
            let texture = gl.create_texture().unwrap();

            gl.bind_texture(gl::TEXTURE_RECTANGLE, Some(texture));
            surface_bind_to_gl_texture(io_surface, size.width, size.height, true);

            gl.tex_parameter_i32(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_MAG_FILTER,
                gl::NEAREST as _,
            );
            gl.tex_parameter_i32(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_MIN_FILTER,
                gl::NEAREST as _,
            );
            gl.tex_parameter_i32(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as _,
            );
            gl.tex_parameter_i32(
                gl::TEXTURE_RECTANGLE,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as _,
            );

            gl.bind_texture(gl::TEXTURE_RECTANGLE, None);

            debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

            texture
        }
    }

    fn destroy_surface_inner(
        &self,
        context: &mut CglContext,
        surface: &mut CglSurface,
    ) -> Result<(), Error> {
        if context.id != surface.context_id() {
            return Err(Error::IncompatibleSurface);
        }
        self.destroy_binding(context, surface.binding_mut()?)
    }

    fn destroy_binding(&self, context: &CglContext, binding: &mut CglBinding) -> Result<(), Error> {
        let gl = &context.gl;

        unsafe {
            if let Some(fbo) = binding.framebuffer_object.take() {
                gl_utils::destroy_framebuffer(gl, fbo);
            }

            binding.renderbuffers.destroy(gl);
            if let Some(texture) = binding.texture_object.take() {
                gl.delete_texture(texture);
            }
        }

        self.0.destroy_surface(&mut binding.system_surface)
    }

    /// Destroys a surface.
    ///
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_window_surface(
        &self,
        context: &mut Context,
        surface: &mut WindowSurface,
    ) -> Result<(), Error> {
        let context: &mut CglContext = context.try_into()?;
        let CglWindowSurface(binding): &mut CglWindowSurface = surface.try_into()?;
        if context.id != binding.context_id {
            return Err(Error::IncompatibleSurface);
        }
        self.destroy_binding(context, binding)
    }

    /// See [`crate::Device::destroy_offscreen_surface`].
    pub fn destroy_offscreen_surface(
        &self,
        context: &mut Context,
        surface: &mut OffscreenSurface,
    ) -> Result<(), Error> {
        let context: &mut CglContext = context.try_into()?;
        let surface: &mut IoSurfaceBuffer = surface.try_into()?;
        if context.id != surface.context_id() {
            return Err(Error::IncompatibleSurface);
        }
        self.destroy_binding(context, surface.cgl_mut()?)
    }

    /// Destroys a surface texture and returns the underlying surface.
    ///
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    ///
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context,
        surface_texture: SurfaceTexture,
    ) -> Result<OffscreenSurface, (Error, SurfaceTexture)> {
        let concrete_context = match context.cgl() {
            Ok(concrete_context) => concrete_context,
            Err(error) => return Err((error, surface_texture)),
        };
        let mut surface_texture: CglSurfaceTexture = surface_texture.try_into()?;

        let gl = &concrete_context.gl;
        if let Some(texture) = surface_texture.texture_object.take() {
            unsafe {
                gl.delete_texture(texture);
            }
        }

        Ok(surface_texture.surface.into())
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> Option<Texture> {
        surface_texture.cgl().ok()?.texture_object
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> u32 {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(
        &self,
        context: &Context,
        surface: &mut WindowSurface,
    ) -> Result<(), Error> {
        let context = context.cgl()?;
        let CglWindowSurface(binding): &mut CglWindowSurface = surface.try_into()?;
        self.0.present_surface(&mut binding.system_surface)?;
        binding.bind_to_texture(&context.gl);
        Ok(())
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        context: &Context,
        surface: &mut WindowSurface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        let concrete_context = context.cgl()?;
        let CglWindowSurface(binding): &mut CglWindowSurface = surface.try_into()?;
        if concrete_context.id != binding.context_id {
            return Err(Error::IncompatibleSurface);
        }

        let _guard = self.temporarily_make_context_current(context.cgl()?);
        let context_descriptor = self.context_descriptor(context);
        let context_attributes = self.context_descriptor_attributes(&context_descriptor);
        self.resize_inner(binding, size, &concrete_context.gl, context_attributes)
    }

    pub(crate) fn resize_inner(
        &self,
        surface: &mut CglBinding,
        size: Size2D<i32>,
        gl: &Rc<gl::Context>,
        context_attributes: ContextAttributes,
    ) -> Result<(), Error> {
        let _guard = self.temporarily_bind_framebuffer(gl.clone(), surface.framebuffer_object);

        self.0.resize_surface(&mut surface.system_surface, size)?;

        unsafe {
            // Recreate the GL texture and bind it to the FBO
            let texture_object =
                self.bind_to_gl_texture(gl, &surface.system_surface.io_surface, &size);
            gl.framebuffer_texture_2d(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                SURFACE_GL_TEXTURE_TARGET,
                Some(texture_object),
                0,
            );

            // Recreate the GL renderbuffers and bind them to the FBO
            let renderbuffers = Renderbuffers::new(gl, &size, &context_attributes);
            renderbuffers.bind_to_current_framebuffer(gl);

            if let Some(texture) = surface.texture_object {
                gl.delete_texture(texture);
            }
            surface.renderbuffers.destroy(gl);

            surface.texture_object = Some(texture_object);
            surface.renderbuffers = renderbuffers;

            debug_assert_eq!(
                (gl.get_error(), gl.check_framebuffer_status(gl::FRAMEBUFFER)),
                (gl::NO_ERROR, gl::FRAMEBUFFER_COMPLETE),
            );
        }

        Ok(())
    }

    fn temporarily_bind_framebuffer(
        &self,
        gl: Rc<Gl>,
        new_framebuffer: Option<glow::Framebuffer>,
    ) -> FramebufferGuard {
        unsafe {
            let current_draw_framebuffer =
                gl.get_parameter_framebuffer(gl::DRAW_FRAMEBUFFER_BINDING);
            let current_read_framebuffer =
                gl.get_parameter_framebuffer(gl::READ_FRAMEBUFFER_BINDING);
            gl.bind_framebuffer(gl::FRAMEBUFFER, new_framebuffer);
            FramebufferGuard {
                gl,
                draw: current_draw_framebuffer,
                read: current_read_framebuffer,
            }
        }
    }

    /// Returns the native `IOSurface` corresponding to this surface.
    ///
    /// The reference count is increased on the `IOSurface` before returning.
    #[inline]
    pub fn native_surface(&self, surface: &Surface) -> NativeSurface {
        #[allow(unreachable_patterns)]
        let binding = match surface {
            Surface::Window(WindowSurface::AppKit(CglWindowSurface(binding))) => Ok(binding),
            Surface::Offscreen(OffscreenSurface::IoSurface(surface)) => surface.cgl(),
            _ => Err(Error::IncompatibleSurface),
        }
        .expect("Incompatible surface type");
        self.0.native_surface(&binding.system_surface)
    }
}

fn make_cgl_context_current(cgl_context: CGLContextObj) -> Result<(), Error> {
    unsafe {
        let err = CGLSetCurrentContext(cgl_context);
        if err != kCGLNoError {
            return Err(Error::MakeCurrentFailed(err.to_windowing_api_error()));
        }
        Ok(())
    }
}

fn get_proc_address(symbol_name: &str) -> *const c_void {
    OPENGL_FRAMEWORK.with(|framework| {
        let symbol_name = CFString::from_str(symbol_name);
        framework.function_pointer_for_name(Some(&symbol_name))
    })
}

#[must_use]
struct FramebufferGuard {
    gl: Rc<Gl>,
    draw: Option<glow::Framebuffer>,
    read: Option<glow::Framebuffer>,
}

impl Drop for FramebufferGuard {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_framebuffer(gl::READ_FRAMEBUFFER, self.read);
            self.gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, self.draw);
        }
    }
}
