//! Wrapper for OSMesa contexts.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::gl::types::GLint;
use crate::gl::{self, Gl};
use crate::info::GLVersion;
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, SurfaceInfo, WindowingApiError};
use super::device::Device;
use super::surface::Surface;

use euclid::default::Size2D;
use osmesa_sys::{self, OSMESA_CONTEXT_MAJOR_VERSION, OSMESA_CONTEXT_MINOR_VERSION};
use osmesa_sys::{OSMESA_COMPAT_PROFILE, OSMESA_CORE_PROFILE, OSMESA_DEPTH_BITS, OSMESA_FORMAT};
use osmesa_sys::{OSMESA_PROFILE, OSMESA_STENCIL_BITS, OSMesaContext, OSMesaCreateContextAttribs};
use osmesa_sys::{OSMesaDestroyContext, OSMesaGetColorBuffer, OSMesaGetCurrentContext};
use osmesa_sys::{OSMesaGetIntegerv, OSMesaGetProcAddress, OSMesaMakeCurrent};
use std::cell::UnsafeCell;
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Arc;
use std::thread;

const DUMMY_FRAMEBUFFER_SIZE: i32 = 16;
const DUMMY_FRAMEBUFFER_AREA: i32 = DUMMY_FRAMEBUFFER_SIZE * DUMMY_FRAMEBUFFER_SIZE;

thread_local! {
    #[doc(hidden)]
    pub static GL_FUNCTIONS: Gl = Gl::load_with(get_proc_address);
}

/// Represents an OpenGL rendering context.
/// 
/// A context allows you to issue rendering commands to a surface. When initially created, a
/// context has no attached surface, so rendering commands will fail or be ignored. Typically, you
/// attach a surface to the context before rendering.
/// 
/// Contexts take ownership of the surfaces attached to them. In order to mutate a surface in any
/// way other than rendering to it (e.g. presenting it to a window, which causes a buffer swap), it
/// must first be detached from its context. Each surface is associated with a single context upon
/// creation and may not be rendered to from any other context. However, you can wrap a surface in
/// a surface texture, which allows the surface to be read from another context.
/// 
/// OpenGL objects may not be shared across contexts directly, but surface textures effectively
/// allow for sharing of texture data. Contexts are local to a single thread and device.
/// 
/// A context must be explicitly destroyed with `destroy_context()`, or a panic will occur.
pub struct Context {
    pub(crate) osmesa_context: OSMesaContext,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<Surface, ()>,
    descriptor: ContextDescriptor,
    status: ContextStatus,
    dummy_pixels: UnsafeCell<Vec<u32>>,
}

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        if self.status != ContextStatus::Destroyed && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

/// Wrapper for a native OSMesa context.
pub struct NativeContext {
    /// The native OSMesa context object.
    pub osmesa_context: OSMesaContext,
    /// Flags that represent attributes of the OSMesa context that we can't automatically detect
    /// from the context, due to API limitations and/or bugs.
    pub flags: NativeContextFlags,
}

bitflags! {
    /// Various flags that represent attributes of the OSMesa context that we can't automatically
    /// detect from the context, due to API limitations and/or bugs.
    /// 
    /// These correspond to a subset of the `ContextAttributeFlags`. Corresponding flags are
    /// guaranteed to have the same bit representation.
    pub struct NativeContextFlags: u8 {
        /// Whether a depth buffer is present.
        const DEPTH   = 0x02;
        /// Whether a stencil buffer is present.
        const STENCIL = 0x04;
        /// Whether the OpenGL compatibility profile is in use. If this is not set, then the core
        /// profile is assumed to be used.
        const COMPATIBILITY_PROFILE = 0x08;
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ContextStatus {
    Owned,
    Referenced,
    Destroyed,
}

/// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
/// 
/// These are local to a device.
#[derive(Clone)]
pub struct ContextDescriptor {
    attributes: Arc<Vec<c_int>>,
}

impl Device {
    /// Creates a context descriptor with the given attributes.
    /// 
    /// Context descriptors are local to this device.
    pub fn create_context_descriptor(&self, attributes: &ContextAttributes)
                                     -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 }; 

        let profile = if flags.contains(ContextAttributeFlags::COMPATIBILITY_PROFILE) ||
                attributes.version.major < 3 {
            OSMESA_COMPAT_PROFILE
        } else {
            OSMESA_CORE_PROFILE
        };

        // We have to use an RGBA format because RGB crashes the Gallium state tracker.
        Ok(ContextDescriptor {
            attributes: Arc::new(vec![
                OSMESA_FORMAT,                  gl::RGBA as i32,
                OSMESA_DEPTH_BITS,              depth_size,
                OSMESA_STENCIL_BITS,            stencil_size,
                OSMESA_PROFILE,                 profile,
                OSMESA_CONTEXT_MAJOR_VERSION,   attributes.version.major as c_int,
                OSMESA_CONTEXT_MINOR_VERSION,   attributes.version.minor as c_int,
                0,
            ]),
        })
    }

    /// Creates a new OpenGL context.
    /// 
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(&mut self, descriptor: &ContextDescriptor) -> Result<Context, Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        unsafe {
            let osmesa_context = OSMesaCreateContextAttribs(descriptor.attributes.as_ptr(),
                                                            ptr::null_mut());
            if osmesa_context.is_null() {
                return Err(Error::ContextCreationFailed(WindowingApiError::BadPixelFormat));
            }

            let context = Context {
                osmesa_context,
                id: *next_context_id,
                framebuffer: Framebuffer::None,
                descriptor: (*descriptor).clone(),
                status: ContextStatus::Owned,
                dummy_pixels: UnsafeCell::new(vec![0; DUMMY_FRAMEBUFFER_AREA as usize]),
            };
            next_context_id.0 += 1;
            Ok(context)
        }
    }

    /// Wraps an existing OSMesa context in a `Context` object.
    ///
    /// The underlying `OSMesaContext` is not retained, as there is no way to do this in the OSMesa
    /// API. Therefore, it is the caller's responsibility to keep it alive as long as this `Context`
    /// remains alive.
    pub unsafe fn create_context_from_native_context(&self, native_context: NativeContext)
                                                     -> Result<Context, Error> {
        // Take a lock.
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Synthesize a context descriptor. The OSMesa API doesn't let us 
        let context_descriptor = GL_FUNCTIONS.with(|gl| {
            // Fetch the current GL version.
            let gl_version = GLVersion::current(gl);

            // Fetch the current image format.
            let mut format = 0;
            OSMesaGetIntegerv(OSMESA_FORMAT, &mut format);

            // Synthesize appropriate attribute values.
            let flags = native_context.flags;
            let depth_size   = if flags.contains(NativeContextFlags::DEPTH)   { 24 } else { 0 };
            let stencil_size = if flags.contains(NativeContextFlags::STENCIL) { 8  } else { 0 }; 

            let profile = if flags.contains(NativeContextFlags::COMPATIBILITY_PROFILE) {
                OSMESA_COMPAT_PROFILE
            } else {
                OSMESA_CORE_PROFILE
            };

            // Create the attributes.
            ContextDescriptor {
                attributes: Arc::new(vec![
                    OSMESA_FORMAT,                  format,
                    OSMESA_DEPTH_BITS,              depth_size,
                    OSMESA_STENCIL_BITS,            stencil_size,
                    OSMESA_PROFILE,                 profile,
                    OSMESA_CONTEXT_MAJOR_VERSION,   gl_version.major as i32,
                    OSMESA_CONTEXT_MINOR_VERSION,   gl_version.minor as i32,
                    0,
                ]),
            }
        });

        let context = Context {
            osmesa_context: native_context.osmesa_context,
            id: *next_context_id,
            framebuffer: Framebuffer::None,
            descriptor: context_descriptor,
            status: ContextStatus::Referenced,
            dummy_pixels: UnsafeCell::new(vec![0; DUMMY_FRAMEBUFFER_AREA as usize]),
        };
        next_context_id.0 += 1;
        Ok(context)
    }

    /// Destroys a context.
    /// 
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.status == ContextStatus::Destroyed {
            return Ok(());
        }

        if let Framebuffer::Surface(mut surface) = mem::replace(&mut context.framebuffer,
                                                                Framebuffer::None) {
            self.destroy_surface(context, &mut surface)?;
        }

        unsafe {
            if context.status == ContextStatus::Owned {
                OSMesaDestroyContext(context.osmesa_context);
            }
        }

        context.osmesa_context = ptr::null_mut();
        context.status = ContextStatus::Destroyed;
        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    #[inline]
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        context.descriptor.clone()
    }

    /// Makes the context the current OpenGL context for this thread.
    /// 
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let (surface_pixels_ptr, surface_size);
            match context.framebuffer {
                Framebuffer::Surface(ref surface) => {
                    surface_pixels_ptr = (*surface.pixels.get()).as_mut_ptr() as *mut c_void;
                    surface_size = surface.size;
                }
                Framebuffer::None => {
                    surface_pixels_ptr =
                        (*context.dummy_pixels.get()).as_mut_ptr() as *mut c_void;
                    surface_size = Size2D::new(DUMMY_FRAMEBUFFER_SIZE, DUMMY_FRAMEBUFFER_SIZE);
                }
                Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
            };

            let ok = OSMesaMakeCurrent(context.osmesa_context,
                                       surface_pixels_ptr,
                                       gl::UNSIGNED_BYTE,
                                       surface_size.width,
                                       surface_size.height);
            if ok == gl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }

            Ok(())
        }
    }

    /// Removes the current OpenGL context from this thread.
    /// 
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            let ok = OSMesaMakeCurrent(ptr::null_mut(), ptr::null_mut(), 0, 0, 0);
            if ok == gl::FALSE {
                return Err(Error::MakeCurrentFailed(WindowingApiError::Failed));
            }
            Ok(())
        }
    }

    /// Returns the attributes that the context descriptor was created with.
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
                (OSMESA_PROFILE, profile) => {
                    if profile == OSMESA_COMPAT_PROFILE as u32 {
                        context_attributes.flags
                                          .insert(ContextAttributeFlags::COMPATIBILITY_PROFILE);
                    }
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

    /// Fetches the address of an OpenGL function associated with this context.
    /// 
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    /// 
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        get_proc_address(symbol_name)
    }

    /// Attaches a surface to a context for rendering.
    /// 
    /// This function takes ownership of the surface. The surface must have been created with this
    /// context, or an `IncompatibleSurface` error is returned.
    /// 
    /// If this function is called with a surface already bound, a `SurfaceAlreadyBound` error is
    /// returned. To avoid this error, first unbind the existing surface with
    /// `unbind_surface_from_context`.
    /// 
    /// If an error is returned, the surface is returned alongside it.
    pub fn bind_surface_to_context(&self, context: &mut Context, surface: Surface)
                                   -> Result<(), (Error, Surface)> {
        if context.id != surface.context_id {
            return Err((Error::IncompatibleSurface, surface));
        }

        match context.framebuffer {
            Framebuffer::None => context.framebuffer = Framebuffer::Surface(surface),
            Framebuffer::External(_) => return Err((Error::ExternalRenderTarget, surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, surface)),
        }

        unsafe {
            if OSMesaGetCurrentContext() == context.osmesa_context {
                drop(self.make_context_current(context));
            }
        }

        Ok(())
    }

    /// Removes and returns any attached surface from this context.
    /// 
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(&self, context: &mut Context)
                                       -> Result<Option<Surface>, Error> {
        match context.framebuffer {
            Framebuffer::None | Framebuffer::Surface(_) => {}
            Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
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
            Framebuffer::External(_) => unreachable!(),
        }
    }

    fn temporarily_make_context_current(&self, context: &Context)
                                        -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    /// Returns a unique ID representing a context.
    /// 
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }

    /// Returns various information about the surface attached to a context.
    /// 
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External(_) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }

    /// Given a context, returns its underlying OSMesa context object.
    #[inline]
    pub fn native_context(&self, context: &Context) -> NativeContext {
        let attributes = self.context_descriptor_attributes(&context.descriptor);

        NativeContext {
            osmesa_context: context.osmesa_context,
            flags: NativeContextFlags::from_bits_truncate(attributes.flags.bits()),
        }
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