//! Surface management for macOS.

use crate::base::io_surface::surface::Surface as SystemSurface;
use crate::context::ContextID;
use crate::renderbuffers::Renderbuffers;
use crate::{gl, SurfaceID};
use cgl::{kCGLNoError, CGLErrorString, CGLGetCurrentContext, CGLTexImageIOSurface2D, GLenum};
use glow::Context as Gl;

use glow::{HasContext, Texture};
use objc2_io_surface::IOSurfaceRef;
use std::ffi::CStr;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::rc::Rc;

pub use crate::base::io_surface::surface::{NativeSurface, NativeWidget};

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
///
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
///
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
pub struct Surface {
    pub(crate) system_surface: SystemSurface,
    pub(crate) context_id: ContextID,
    pub(crate) framebuffer_object: Option<glow::Framebuffer>,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) renderbuffers: Renderbuffers,
}

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) texture_object: Option<Texture>,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Debug for SurfaceTexture {
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

impl Surface {
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
}
