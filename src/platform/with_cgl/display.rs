use crate::gl_context::GLVersion;
use crate::gl_formats::Format;
use crate::platform::with_cgl::{NativeSurface, NativeSurfaceTexture};
use crate::surface::SurfaceDescriptor;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl, GlType};
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement};
use io_surface::{kIOSurfaceBytesPerRow, kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Clone)]
pub struct Display {
    phantom: PhantomData<*mut ()>,
}

pub type NativeDisplay = ();

impl Display {
    #[inline]
    pub fn new() -> Display {
        Display { phantom: PhantomData }
    }

    #[inline]
    pub fn from_native_display(_: ()) -> Display {
        Display::new()
    }

    pub fn create_surface_from_descriptor(&self, gl: &dyn Gl, descriptor: &SurfaceDescriptor)
                                          -> NativeSurface {
        let io_surface = unsafe {
            let props = CFDictionary::from_CFType_pairs(&[
                (CFString::wrap_under_get_rule(kIOSurfaceWidth),
                 CFNumber::from(descriptor.size.width).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceHeight),
                 CFNumber::from(descriptor.size.height).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                 CFNumber::from(BYTES_PER_PIXEL).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                 CFNumber::from(descriptor.size.width * BYTES_PER_PIXEL).as_CFType()),
            ]);
            io_surface::new(&props)
        };

        NativeSurface { io_surface, descriptor: Arc::new(*descriptor) }
    }

    pub fn create_surface_texture(&self, gl: &dyn Gl, native_surface: NativeSurface)
                                  -> NativeSurfaceTexture {
        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_RECTANGLE_ARB, texture);

        let descriptor = native_surface.descriptor();
        let (size, alpha) = (descriptor.size, descriptor.format.has_alpha());
        native_surface.io_surface.bind_to_gl_texture(size.width, size.height, alpha);

        // Low filtering to allow rendering
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_MAG_FILTER,
                           gl::NEAREST as GLint);
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_MIN_FILTER,
                           gl::NEAREST as GLint);

        // TODO(emilio): Check if these two are neccessary, probably not
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_WRAP_S,
                           gl::CLAMP_TO_EDGE as GLint);
        gl.tex_parameter_i(gl::TEXTURE_RECTANGLE_ARB,
                           gl::TEXTURE_WRAP_T,
                           gl::CLAMP_TO_EDGE as GLint);

        gl.bind_texture(gl::TEXTURE_RECTANGLE_ARB, 0);

        debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

        NativeSurfaceTexture { surface: native_surface, gl_texture: texture, phantom: PhantomData }
    }

    pub fn destroy_surface_texture(&self,
                                   gl: &dyn Gl,
                                   mut surface_texture: NativeSurfaceTexture)
                                   -> NativeSurface {
        gl.delete_textures(&[surface_texture.gl_texture]);
        surface_texture.gl_texture = 0;
        surface_texture.surface
    }
}
