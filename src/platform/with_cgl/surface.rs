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
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow};
use io_surface::{kIOSurfaceHeight, kIOSurfaceIsGlobal, kIOSurfaceWidth};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Formatter};
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Clone)]
pub struct SerializableIOSurface(IOSurface);

// FIXME(pcwalton): We should turn the IOSurface into a Mach port instead of using global IDs.
impl Serialize for SerializableIOSurface {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_u32(self.0.get_id())
    }
}

// FIXME(pcwalton): We should turn the IOSurface into a Mach port instead of using global IDs.
impl<'de> Deserialize<'de> for SerializableIOSurface {
    fn deserialize<D>(d: D) -> Result<SerializableIOSurface, D::Error> where D: Deserializer<'de> {
        Ok(SerializableIOSurface(io_surface::lookup(Deserialize::deserialize(d)?)))
    }
}

pub struct TransientGLTexture(GLuint);

impl Serialize for TransientGLTexture {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_unit()
    }
}

impl<'de> Deserialize<'de> for TransientGLTexture {
    fn deserialize<D>(d: D) -> Result<TransientGLTexture, D::Error> where D: Deserializer<'de> {
        let () = Deserialize::deserialize(d)?;
        Ok(TransientGLTexture(0))
    }
}

#[allow(unsafe_code)]
unsafe impl Send for SerializableIOSurface {}

#[derive(Serialize, Deserialize)]
pub struct NativeSurface {
    io_surface: SerializableIOSurface,
    size: Size2D<i32>,
    alpha: bool,
    gl_texture: TransientGLTexture,
}

impl Debug for NativeSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}, {:?}", self.size, self.alpha)
    }
}

impl Drop for NativeSurface {
    fn drop(&mut self) {
        debug_assert!(self.gl_texture.0 == 0 || thread::panicking(),    
                      "Must destroy the native surface manually!");
    }
}

impl NativeSurface {
    pub fn new(gl: &dyn Gl, size: &Size2D<i32>, formats: &GLFormats) -> NativeSurface {
        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_RECTANGLE_ARB, texture);
        let has_alpha = formats.has_alpha();

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
                (CFString::wrap_under_get_rule(kIOSurfaceIsGlobal),
                 CFBoolean::from(true).as_CFType()),
            ]);
            io_surface::new(&props)
        };

        io_surface.bind_to_gl_texture(size.width, size.height, has_alpha);

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

        NativeSurface {
            io_surface: SerializableIOSurface(io_surface),
            size: *size,
            alpha: has_alpha,
            gl_texture: TransientGLTexture(texture),
        }
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn alpha(&self) -> bool {
        self.alpha
    }

    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture.0
    }

    #[inline]
    pub fn gl_texture_type(&self) -> GLenum {
        gl::TEXTURE_RECTANGLE_ARB
    }

    #[inline]
    pub fn destroy(&mut self, gl: &dyn Gl) {
        gl.delete_textures(&[self.gl_texture.0]);
        self.gl_texture.0 = 0;
    }
}
