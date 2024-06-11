// surfman/surfman/src/gl_utils.rs
//
//! Various OpenGL utilities used by the different backends.

use crate::gl;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::Gl;

#[allow(dead_code)]
pub(crate) fn create_and_bind_framebuffer(
    gl: &Gl,
    texture_target: GLenum,
    texture_object: GLuint,
) -> GLuint {
    unsafe {
        let mut framebuffer_object = 0;
        gl.GenFramebuffers(1, &mut framebuffer_object);
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        gl.FramebufferTexture2D(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            texture_target,
            texture_object,
            0,
        );
        framebuffer_object
    }
}

pub(crate) fn unbind_framebuffer_if_necessary(gl: &Gl, framebuffer_object: GLuint) {
    unsafe {
        // Unbind the framebuffer if it's bound.
        let (mut current_draw_framebuffer, mut current_read_framebuffer) = (0, 0);
        gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut current_draw_framebuffer);
        gl.GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut current_read_framebuffer);
        if current_draw_framebuffer == framebuffer_object as GLint {
            gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
        }
        if current_read_framebuffer == framebuffer_object as GLint {
            gl.BindFramebuffer(gl::READ_FRAMEBUFFER, 0);
        }
    }
}

#[allow(dead_code)]
pub(crate) fn destroy_framebuffer(gl: &Gl, framebuffer_object: GLuint) {
    unbind_framebuffer_if_necessary(gl, framebuffer_object);
    unsafe {
        gl.DeleteFramebuffers(1, &framebuffer_object);
    }
}
