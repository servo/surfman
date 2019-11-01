//! Wrapper for OSMesa contexts.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::gl::types::{GLint, GLuint};
use crate::gl::{self, Gl};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, SurfaceAccess, SurfaceID};
use crate::{SurfaceType, WindowingApiError};
use super::device::Device;
use super::surface::{NativeWidget, Surface};

use euclid::default::Size2D;
use osmesa_sys::{self, OSMESA_CONTEXT_MAJOR_VERSION, OSMESA_CONTEXT_MINOR_VERSION};
use osmesa_sys::{OSMESA_COMPAT_PROFILE, OSMESA_CORE_PROFILE, OSMESA_DEPTH_BITS, OSMESA_FORMAT};
use osmesa_sys::{OSMESA_PROFILE, OSMESA_STENCIL_BITS, OSMesaContext, OSMesaCreateContextAttribs};
use osmesa_sys::{OSMesaDestroyContext, OSMesaGetColorBuffer, OSMesaGetCurrentContext};
use osmesa_sys::{OSMesaGetDepthBuffer, OSMesaGetIntegerv, OSMesaGetProcAddress, OSMesaMakeCurrent};
use std::ffi::CString;
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Arc;
use std::thread;

thread_local! {
    pub static GL_FUNCTIONS: Gl = Gl::load_with(get_proc_address);
}

pub struct Context {
    pub(crate) native_context: Box<dyn NativeContext>,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<Surface>,
}

pub(crate) trait NativeContext {
    fn osmesa_context(&self) -> OSMesaContext;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self, device: &Device);
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if !self.native_context.is_destroyed() && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

#[derive(Clone)]
pub struct ContextDescriptor {
    attributes: Arc<Vec<c_int>>,
}

impl Context {
    pub fn id(&self) -> ContextID {
        self.id
    }
}

impl Device {
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;
        let format = if flags.contains(ContextAttributeFlags::ALPHA) { gl::RGBA } else { gl::RGB };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 }; 

        let profile = if flags.contains(ContextAttributeFlags::COMPATIBILITY_PROFILE) ||
                attributes.version.major < 3 {
            OSMESA_COMPAT_PROFILE
        } else {
            OSMESA_CORE_PROFILE
        };

        Ok(ContextDescriptor {
            attributes: Arc::new(vec![
                OSMESA_FORMAT,                  format as i32,
                OSMESA_DEPTH_BITS,              depth_size,
                OSMESA_STENCIL_BITS,            stencil_size,
                OSMESA_PROFILE,                 profile,
                OSMESA_CONTEXT_MAJOR_VERSION,   attributes.version.major as c_int,
                OSMESA_CONTEXT_MINOR_VERSION,   attributes.version.minor as c_int,
                0,
            ]),
        })
    }

    /// Opens the device and context corresponding to the current OSMesa context.
    ///
    /// The native context is not retained, as there is no way to do this in the OSMesa API. It is
    /// the caller's responsibility to keep it alive for the duration of this context. Be careful
    /// when using this method; it's essentially a last resort.
    ///
    /// This method is designed to allow `surfman` to deal with contexts created outside the
    /// library. It's legal to use this method to wrap a context rendering to any target. The
    /// target is opaque to `surfman`; the library will not modify or try to detect the render
    /// target. As a consequence, any of the methods that query or replace the surface—e.g.
    /// `replace_context_surface`—will fail if called with a context object created via this
    /// method.
    pub unsafe fn from_current_context() -> Result<(Device, Context), Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Create a device.
        let device = Device { phantom: PhantomData };

        // Get the current context.
        let osmesa_context = OSMesaGetCurrentContext();
        assert!(!osmesa_context.is_null());

        // Wrap the context.
        let context = Context {
            native_context: Box::new(UnsafeOSMesaContextRef { osmesa_context }),
            id: *next_context_id,
            framebuffer: Framebuffer::External,
        };
        next_context_id.0 += 1;

        Ok((device, context))
    }

    pub fn create_context(&mut self, descriptor: &ContextDescriptor) -> Result<Context, Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        unsafe {
            let osmesa_context = OSMesaCreateContextAttribs(descriptor.attributes.as_ptr(),
                                                            ptr::null_mut());
            if osmesa_context.is_null() {
                return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
            }

            let mut context = Context {
                native_context: Box::new(OwnedOSMesaContext { osmesa_context }),
                id: *next_context_id,
                framebuffer: Framebuffer::None,
            };
            next_context_id.0 += 1;
            Ok(context)
        }
    }

    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.native_context.is_destroyed() {
            return Ok(());
        }

        if let Framebuffer::Surface(surface) = mem::replace(&mut context.framebuffer,
                                                            Framebuffer::None) {
            self.destroy_surface(context, surface)?;
        }

        unsafe {
            context.native_context.destroy(self);
        }

        Ok(())
    }

    // FIXME(pcwalton): Probably should return a result here to avoid an unwrap.
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        self.make_context_current(context).unwrap();

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                // Fetch the current GL version.
                let (mut major_gl_version, mut minor_gl_version) = (0, 0);
                gl.GetIntegerv(gl::MAJOR_VERSION, &mut major_gl_version);
                gl.GetIntegerv(gl::MINOR_VERSION, &mut minor_gl_version);

                // Fetch the current image format.
                let mut format = 0;
                OSMesaGetIntegerv(OSMESA_FORMAT, &mut format);

                // Fetch the depth size.
                let (mut depth_width, mut depth_height, mut depth_byte_size) = (0, 0, 0);
                let mut depth_buffer = ptr::null_mut();
                let has_depth = OSMesaGetDepthBuffer(context.native_context.osmesa_context(),
                                                    &mut depth_width,
                                                    &mut depth_height,
                                                    &mut depth_byte_size,
                                                    &mut depth_buffer);
                if has_depth == gl::FALSE {
                    depth_byte_size = 0;
                }

                let profile = if major_gl_version < 3 { OSMESA_COMPAT_PROFILE } else { OSMESA_CORE_PROFILE };

                // Create a set of attributes.
                //
                // FIXME(pcwalton): I don't see a way to get the current stencil size in the OSMesa
                // API. Just guess, I suppose.
                // FIXME(pcwalton): How does OSMesa deal with packed depth/stencil?
                ContextDescriptor {
                    attributes: Arc::new(vec![
                        OSMESA_FORMAT,                  format,
                        OSMESA_DEPTH_BITS,              depth_byte_size * 8,
                        OSMESA_STENCIL_BITS,            8,
                        OSMESA_PROFILE,                 profile,
                        OSMESA_CONTEXT_MAJOR_VERSION,   major_gl_version,
                        OSMESA_CONTEXT_MINOR_VERSION,   minor_gl_version,
                        0,
                    ]),
                }
            }
        })
    }

    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let surface = match context.framebuffer {
                Framebuffer::Surface(ref surface) => surface,
                Framebuffer::None | Framebuffer::External => {
                    return Err(Error::ExternalRenderTarget)
                }
            };

            let ok = OSMesaMakeCurrent(context.native_context.osmesa_context(),
                                       (*surface.pixels.get()).as_mut_ptr() as *mut c_void,
                                       gl::UNSIGNED_BYTE,
                                       surface.size.width,
                                       surface.size.height);
            if ok == gl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }

            Ok(())
        }
    }

    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            let ok = OSMesaMakeCurrent(ptr::null_mut(), ptr::null_mut(), 0, 0, 0);
            if ok == gl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }
            Ok(())
        }
    }

    #[inline]
    pub fn context_surface<'c>(&self, context: &'c Context) -> Result<Option<&'c Surface>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(surface)),
        }
    }

    pub fn context_descriptor_attributes(&self, context_descriptor: &ContextDescriptor)
                                         -> ContextAttributes {
        let mut context_attributes = ContextAttributes::zeroed();
        for attribute_value_pair in context_descriptor.attributes.chunks(2) {
            if attribute_value_pair.len() < 2 || attribute_value_pair[0] == 0 {
                break;
            }
            match (attribute_value_pair[0], attribute_value_pair[1] as u32) {
                (OSMESA_FORMAT, gl::RGBA) => {
                    context_attributes.flags.insert(ContextAttributeFlags::ALPHA)
                }
                (OSMESA_DEPTH_BITS, 0) => {}
                (OSMESA_DEPTH_BITS, _) => {
                    context_attributes.flags.insert(ContextAttributeFlags::DEPTH)
                }
                (OSMESA_STENCIL_BITS, 0) => {}
                (OSMESA_STENCIL_BITS, _) => {
                    context_attributes.flags.insert(ContextAttributeFlags::STENCIL)
                }
                (OSMESA_CONTEXT_MAJOR_VERSION, major_version) => {
                    context_attributes.version.major = major_version as u8
                }
                (OSMESA_CONTEXT_MINOR_VERSION, minor_version) => {
                    context_attributes.version.minor = minor_version as u8
                }
                _ => {}
            }
        }

        context_attributes
    }

    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        get_proc_address(symbol_name)
    }

    pub fn bind_surface_to_context(&self, context: &mut Context, surface: Surface)
                                   -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        match context.framebuffer {
            Framebuffer::None => {
                context.framebuffer = Framebuffer::Surface(surface);
                Ok(())
            }
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(_) => Err(Error::SurfaceAlreadyBound),
        }
    }

    pub fn unbind_surface_from_context(&self, context: &mut Context)
                                       -> Result<Option<Surface>, Error> {
        match context.framebuffer {
            Framebuffer::None | Framebuffer::Surface(_) => {}
            Framebuffer::External => return Err(Error::ExternalRenderTarget),
        }

        let _guard = self.temporarily_make_context_current(context)?;

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.Flush();
            }
        });

        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => Ok(Some(surface)),
            Framebuffer::None => Ok(None),
            Framebuffer::External => unreachable!(),
        }
    }

    fn temporarily_make_context_current(&self, context: &Context)
                                        -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }
}

struct OwnedOSMesaContext {
    osmesa_context: OSMesaContext,
}

impl NativeContext for OwnedOSMesaContext {
    #[inline]
    fn osmesa_context(&self) -> OSMesaContext {
        debug_assert!(!self.is_destroyed());
        self.osmesa_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.osmesa_context.is_null()
    }

    unsafe fn destroy(&mut self, _: &Device) {
        assert!(!self.is_destroyed());
        OSMesaDestroyContext(self.osmesa_context);
        self.osmesa_context = ptr::null_mut();
    }
}

struct UnsafeOSMesaContextRef {
    osmesa_context: OSMesaContext,
}

impl NativeContext for UnsafeOSMesaContextRef {
    #[inline]
    fn osmesa_context(&self) -> OSMesaContext {
        self.osmesa_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.osmesa_context.is_null()
    }

    unsafe fn destroy(&mut self, _: &Device) {
        assert!(!self.is_destroyed());
        self.osmesa_context = ptr::null_mut();
    }
}

fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_name = symbol_name.as_ptr() as *const u8 as *const c_char;
        match OSMesaGetProcAddress(symbol_name) {
            Some(pointer) => pointer as *const c_void,
            None => ptr::null(),
        }
    }
}

#[must_use]
struct CurrentContextGuard {
    old_osmesa_context: OSMesaContext,
    old_width: GLint,
    old_height: GLint,
    old_buffer: *mut c_void,
}

impl Drop for CurrentContextGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            OSMesaMakeCurrent(self.old_osmesa_context,
                              self.old_buffer,
                              gl::UNSIGNED_BYTE,
                              self.old_width,
                              self.old_height);
        }
    }
}

impl CurrentContextGuard {
    pub(crate) fn new() -> CurrentContextGuard {
        unsafe {
            let osmesa_context = OSMesaGetCurrentContext();
            let (mut width, mut height, mut format, mut buffer) = (0, 0, 0, ptr::null_mut());
            if !osmesa_context.is_null() {
                OSMesaGetColorBuffer(osmesa_context,
                                     &mut width,
                                     &mut height,
                                     &mut format,
                                     &mut buffer);
            }
            CurrentContextGuard {
                old_osmesa_context: osmesa_context,
                old_width: width,
                old_height: height,
                old_buffer: buffer,
            }
        }
    }
}