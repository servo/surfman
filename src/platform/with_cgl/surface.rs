/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::gl_formats::GLFormats;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl};
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement};
use io_surface::{kIOSurfaceBytesPerRow, kIOSurfaceHeight, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Clone)]
pub struct NativeSurface {
    io_surface: IOSurface,
    size: Size2D<i32>,
    formats: GLFormats,
}

#[derive(Debug)]
pub struct NativeSurfaceTexture {
    surface: NativeSurface,
    gl_texture: GLuint,
    phantom: PhantomData<*const ()>,
}

unsafe impl Send for NativeSurface {}

impl Debug for NativeSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}, {:?}", self.size, self.formats)
    }
}

impl NativeSurface {
    pub fn new(gl: &dyn Gl, size: &Size2D<i32>, formats: &GLFormats) -> NativeSurface {
        let io_surface = unsafe {
            let props = CFDictionary::from_CFType_pairs(&[
                (CFString::wrap_under_get_rule(kIOSurfaceWidth),
                 CFNumber::from(size.width).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceHeight),
                 CFNumber::from(size.height).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                 CFNumber::from(BYTES_PER_PIXEL).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                 CFNumber::from(size.width * BYTES_PER_PIXEL).as_CFType()),
            ]);
            io_surface::new(&props)
        };

        NativeSurface { io_surface, size: *size, formats: *formats }
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn formats(&self) -> &GLFormats {
        &self.formats
    }

    #[inline]
    pub fn id(&self) -> u32 {
        self.io_surface.get_id()
    }
}

impl NativeSurfaceTexture {
    pub fn new(gl: &dyn Gl, native_surface: NativeSurface) -> NativeSurfaceTexture {
        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_RECTANGLE_ARB, texture);

        let (size, alpha) = (native_surface.size(), native_surface.formats().has_alpha());
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

    #[inline]
    pub fn surface(&self) -> &NativeSurface {
        &self.surface
    }

    #[inline]
    pub fn into_surface(mut self, gl: &dyn Gl) -> NativeSurface {
        self.destroy(gl);
        self.surface
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }

    #[inline]
    pub fn gl_texture_target(&self) -> GLenum {
        gl::TEXTURE_RECTANGLE_ARB
    }

    #[inline]
    pub fn destroy(&mut self, gl: &dyn Gl) {
        gl.delete_textures(&[self.gl_texture]);
        self.gl_texture = 0;
    }
}
