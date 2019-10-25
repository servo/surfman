// surfman/surfman/src/platform/generic/egl/surface.rs
//
//! Functionality common to backends using EGL surfaces.

use crate::egl::types::{EGLConfig, EGLDisplay, EGLImageKHR, EGLSurface, EGLint};
use crate::egl;
use crate::gl::types::{GLint, GLuint};
use crate::gl::{self, Gl};
use super::ffi::EGL_EXTENSION_FUNCTIONS;

use euclid::default::Size2D;

pub(crate) unsafe fn create_pbuffer_surface(egl_display: EGLDisplay,
                                            egl_config: EGLConfig,
                                            size: &Size2D<i32>)
                                            -> EGLSurface {
    let attributes = [
        egl::WIDTH as EGLint,           size.width as EGLint,
        egl::HEIGHT as EGLint,          size.height as EGLint,
        egl::TEXTURE_FORMAT as EGLint,  egl::TEXTURE_RGBA as EGLint,
        egl::TEXTURE_TARGET as EGLint,  egl::TEXTURE_2D as EGLint,
        egl::NONE as EGLint,            0,
        0,                              0,
    ];

    let egl_surface = egl::CreatePbufferSurface(egl_display, egl_config, attributes.as_ptr());
    assert_ne!(egl_surface, egl::NO_SURFACE);
    egl_surface
}

pub(crate) unsafe fn bind_egl_image_to_gl_texture(gl: &Gl, egl_image: EGLImageKHR) -> GLuint {
    let mut texture = 0;
    gl.GenTextures(1, &mut texture);
    debug_assert_ne!(texture, 0);

    // FIXME(pcwalton): Should this be `GL_TEXTURE_EXTERNAL_OES`?
    gl.BindTexture(gl::TEXTURE_2D, texture);
    (EGL_EXTENSION_FUNCTIONS.ImageTargetTexture2DOES)(gl::TEXTURE_2D, egl_image);
    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
    gl.BindTexture(gl::TEXTURE_2D, 0);

    debug_assert_eq!(gl.GetError(), gl::NO_ERROR);
    texture
}

