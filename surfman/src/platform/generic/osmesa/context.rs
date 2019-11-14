//! Wrapper for OSMesa contexts.

use crate::context::{CREATE_CONTEXT_MUTEX, ContextID};
use crate::gl::types::GLint;
use crate::gl::{self, Gl};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, Error, SurfaceInfo, WindowingApiError};
use super::device::Device;
use super::surface::Surface;

use osmesa_sys::{self, OSMESA_CONTEXT_MAJOR_VERSION, OSMESA_CONTEXT_MINOR_VERSION};
use osmesa_sys::{OSMESA_COMPAT_PROFILE, OSMESA_CORE_PROFILE, OSMESA_DEPTH_BITS, OSMESA_FORMAT};
use osmesa_sys::{OSMESA_PROFILE, OSMESA_STENCIL_BITS, OSMesaContext, OSMesaCreateContextAttribs};
use osmesa_sys::{OSMesaDestroyContext, OSMesaGetColorBuffer, OSMesaGetCurrentContext};
use osmesa_sys::{OSMesaGetDepthBuffer, OSMesaGetIntegerv, OSMesaGetProcAddress, OSMesaMakeCurrent};
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Arc;
use std::thread;

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
    framebuffer: Framebuffer<Surface>,
    status: ContextStatus,
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
pub struct NativeContext(pub OSMesaContext);

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
                return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
            }

            let context = Context {
                osmesa_context,
                id: *next_context_id,
                framebuffer: Framebuffer::None,
                status: ContextStatus::Owned,
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

        let context = Context {
            osmesa_context: native_context.0,
            id: *next_context_id,
            framebuffer: Framebuffer::None,
            status: ContextStatus::Referenced,
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
                let has_depth = OSMesaGetDepthBuffer(context.osmesa_context,
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

    /// Makes the context the current OpenGL context for this thread.
    /// 
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let surface = match context.framebuffer {
                Framebuffer::Surface(ref surface) => surface,
                Framebuffer::None | Framebuffer::External => {
                    return Err(Error::ExternalRenderTarget)
                }
            };

            let ok = OSMesaMakeCurrent(context.osmesa_context,
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
            Framebuffer::None => {
                context.framebuffer = Framebuffer::Surface(surface);
                Ok(())
            }
            Framebuffer::External => Err((Error::ExternalRenderTarget, surface)),
            Framebuffer::Surface(_) => Err((Error::SurfaceAlreadyBound, surface)),
        }
    }

    /// Removes and returns any attached surface from this context.
    /// 
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
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
            Framebuffer::External => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
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