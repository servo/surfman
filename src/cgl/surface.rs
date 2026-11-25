//! Surface management for macOS.

use crate::base::io_surface::buffer::IoSurfaceBinding;
use crate::base::io_surface::surface::Surface as SystemSurface;
use crate::context::ContextID;
use crate::renderbuffers::Renderbuffers;
use crate::{gl, Error, OffscreenSurface, Surface, SurfaceID, SurfaceInfo, WindowSurface};
use cgl::{kCGLNoError, CGLErrorString, CGLGetCurrentContext, CGLTexImageIOSurface2D, GLenum};
use glow::Context as Gl;

use glow::{HasContext, Texture};
use objc2_io_surface::IOSurfaceRef;
use std::ffi::CStr;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::rc::Rc;

pub use crate::base::io_surface::buffer::IoSurfaceBuffer;
pub use crate::base::io_surface::surface::NativeSurface;

/// An implementation of [`crate::Surface`] for CGL platforms.
#[derive(Debug)]
pub(crate) enum CglSurface {
    /// A surface displayed in an AppKit view.
    Window(CglWindowSurface),
    /// An `IOSurface` that can be bound to a texture.
    Buffer(IoSurfaceBuffer),
}

/// A CGL surface displayed in an AppKit view.
pub struct CglWindowSurface(pub(crate) CglBinding);

/// The CGL objects that render to the `IOSurface` of a surface.
pub(crate) struct CglBinding {
    pub(crate) system_surface: SystemSurface,
    pub(crate) context_id: ContextID,
    pub(crate) framebuffer_object: Option<glow::Framebuffer>,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) renderbuffers: Renderbuffers,
}

/// An implementation of [`crate::SurfaceTexture`] for CGL platforms.
pub struct CglSurfaceTexture {
    pub(crate) surface: IoSurfaceBuffer,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for CglWindowSurface {}

impl Debug for CglWindowSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "CglWindowSurface({:x})", self.0.id().0)
    }
}

impl TryFrom<Surface> for CglSurface {
    type Error = (Error, Surface);
    fn try_from(surface: Surface) -> Result<CglSurface, Self::Error> {
        #[allow(unreachable_patterns)]
        match surface {
            Surface::Window(WindowSurface::AppKit(surface)) => Ok(CglSurface::Window(surface)),
            Surface::Offscreen(OffscreenSurface::IoSurface(surface)) if surface.cgl().is_ok() => {
                Ok(CglSurface::Buffer(surface))
            }
            surface => Err((Error::IncompatibleSurface, surface)),
        }
    }
}

impl From<CglSurface> for Surface {
    fn from(surface: CglSurface) -> Surface {
        match surface {
            CglSurface::Window(surface) => Surface::Window(surface.into()),
            CglSurface::Buffer(surface) => Surface::Offscreen(surface.into()),
        }
    }
}

impl From<CglBinding> for IoSurfaceBuffer {
    fn from(binding: CglBinding) -> IoSurfaceBuffer {
        IoSurfaceBuffer {
            io_surface: binding.system_surface.io_surface.clone(),
            binding: IoSurfaceBinding::Cgl(binding),
        }
    }
}

impl Debug for CglSurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

pub(crate) fn surface_bind_to_gl_texture(
    surface: &IOSurfaceRef,
    width: i32,
    height: i32,
    has_alpha: bool,
) {
    const BGRA: GLenum = 0x80E1;
    const RGBA: GLenum = 0x1908;
    const RGB: GLenum = 0x1907;
    const TEXTURE_RECTANGLE_ARB: GLenum = 0x84F5;
    const UNSIGNED_INT_8_8_8_8_REV: GLenum = 0x8367;

    unsafe {
        let context = CGLGetCurrentContext();
        let gl_error = CGLTexImageIOSurface2D(
            context,
            TEXTURE_RECTANGLE_ARB,
            if has_alpha {
                RGBA as GLenum
            } else {
                RGB as GLenum
            },
            width,
            height,
            BGRA as GLenum,
            UNSIGNED_INT_8_8_8_8_REV,
            surface as *const IOSurfaceRef as cgl::IOSurfaceRef,
            0,
        );

        if gl_error != kCGLNoError {
            let error_msg = CStr::from_ptr(CGLErrorString(gl_error));
            panic!("{}", error_msg.to_string_lossy());
        }
    }
}

impl CglSurface {
    pub(crate) fn binding_mut(&mut self) -> Result<&mut CglBinding, Error> {
        match self {
            CglSurface::Window(surface) => Ok(&mut surface.0),
            CglSurface::Buffer(surface) => surface.cgl_mut(),
        }
    }

    pub(crate) fn context_id(&self) -> ContextID {
        match self {
            CglSurface::Window(surface) => surface.0.context_id,
            CglSurface::Buffer(surface) => surface.context_id(),
        }
    }

    /// Returns various information about the surface.
    #[inline]
    pub fn info(&self) -> SurfaceInfo {
        match self {
            CglSurface::Window(surface) => surface.0.info(),
            CglSurface::Buffer(surface) => surface.info(),
        }
    }
}

impl CglBinding {
    #[inline]
    pub(crate) fn id(&self) -> SurfaceID {
        SurfaceID(&*self.system_surface.io_surface as *const IOSurfaceRef as usize)
    }

    pub(crate) fn bind_to_texture(&self, gl: &Rc<Gl>) {
        let size = self.system_surface.size;
        unsafe { gl.bind_texture(gl::TEXTURE_RECTANGLE, self.texture_object) };
        surface_bind_to_gl_texture(
            &self.system_surface.io_surface,
            size.width,
            size.height,
            true,
        );
        unsafe { gl.bind_texture(gl::TEXTURE_RECTANGLE, None) };
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.system_surface.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: self.framebuffer_object,
        }
    }
}
