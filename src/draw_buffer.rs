use euclid::default::Size2D;
use gleam::gl;
use gleam::gl::types::{GLuint, GLenum, GLint};
use std::rc::Rc;

use crate::GLContext;
use crate::NativeGLContextMethods;

#[cfg(target_os="macos")]
use io_surface::IOSurfaceID;
#[cfg(target_os="macos")]
use core_foundation::base::TCFType;
#[cfg(target_os="macos")]
use core_foundation::dictionary::CFDictionary;
#[cfg(target_os="macos")]
use core_foundation::string::CFString;
#[cfg(target_os="macos")]
use core_foundation::number::CFNumber;
#[cfg(target_os="macos")]
use core_foundation::boolean::CFBoolean;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ColorAttachmentType {
    Texture,
    Renderbuffer,
    #[cfg(target_os="macos")]
    IOSurface,
}

impl Default for ColorAttachmentType {
    fn default() -> ColorAttachmentType {
        ColorAttachmentType::Renderbuffer
    }
}

/// We either have a color renderbuffer, or a surface bound to a texture bound
/// to a framebuffer as a color attachment.
///
/// NB: The draw buffer manages it, and calls its destroy method on drop, this
/// is just to avoid propagating the GL functions pointer further down.
#[derive(Debug)]
pub enum ColorAttachment {
    Renderbuffer(GLuint),
    Texture(GLuint),
    #[cfg(target_os="macos")]
    IOSurface(GLuint, IOSurfaceID),
}

impl ColorAttachment {
    pub fn color_attachment_type(&self) -> ColorAttachmentType {
        match *self {
            ColorAttachment::Renderbuffer(_) => ColorAttachmentType::Renderbuffer,
            ColorAttachment::Texture(_) => ColorAttachmentType::Texture,
            ColorAttachment::IOSurface(..) => ColorAttachmentType::IOSurface,
        }
    }

    fn destroy(self, gl: &dyn gl::Gl) {
        match self {
            ColorAttachment::Renderbuffer(id) => gl.delete_renderbuffers(&[id]),
            ColorAttachment::Texture(tex_id) => gl.delete_textures(&[tex_id]),
            ColorAttachment::IOSurface(tex_id, _) => gl.delete_textures(&[tex_id]),
        }
    }
}

/// This structure represents an offscreen context
/// draw buffer. It has a framebuffer, with at least
/// color renderbuffer (alpha or not). It may also have
/// packed or independent depth or stencil buffers,
/// depending on context requirements.
pub struct DrawBuffer {
    gl_: Rc<dyn gl::Gl>,
    size: Size2D<i32>,
    framebuffer: GLuint,
    color_attachment: Option<ColorAttachment>,
    stencil_renderbuffer: GLuint,
    depth_renderbuffer: GLuint,
    packed_depth_stencil_renderbuffer: GLuint,
    // samples: GLsizei,
}

/// Helper function to create a render buffer
/// TODO(emilio): We'll need to switch between `glRenderbufferStorage` and
/// `glRenderbufferStorageMultisample` when we support antialising
fn create_renderbuffer(gl_: &dyn gl::Gl,
                       format: GLenum,
                       size: &Size2D<i32>) -> GLuint {
    let ret = gl_.gen_renderbuffers(1)[0];
    gl_.bind_renderbuffer(gl::RENDERBUFFER, ret);
    gl_.renderbuffer_storage(gl::RENDERBUFFER, format, size.width, size.height);
    gl_.bind_renderbuffer(gl::RENDERBUFFER, 0);

    ret
}

impl DrawBuffer {
    pub fn new<T: NativeGLContextMethods>(context: &GLContext<T>,
                                          mut size: Size2D<i32>,
                                          color_attachment_type: ColorAttachmentType)
                                          -> Result<Self, &'static str>
    {
        const MIN_DRAWING_BUFFER_SIZE: i32 = 16;
        use std::cmp;

        let attrs = context.borrow_attributes();
        let capabilities = context.borrow_capabilities();

        debug!("Creating draw buffer {:?}, {:?}, attrs: {:?}, caps: {:?}",
               size, color_attachment_type, attrs, capabilities);

        // WebGL spec: antialias attribute is a requests, not a requirement.
        // If not supported it shall not cause a failure to create a WebGLRenderingContext.
        if attrs.antialias && capabilities.max_samples == 0 {
            error!("The given GLContext doesn't support requested antialising");
        }

        if attrs.preserve_drawing_buffer {
            return Err("preserveDrawingBuffer is not supported yet");
        }

        // See https://github.com/servo/servo/issues/12320
        size.width = cmp::max(MIN_DRAWING_BUFFER_SIZE, size.width);
        size.height = cmp::max(MIN_DRAWING_BUFFER_SIZE, size.height);

        let mut draw_buffer = DrawBuffer {
            gl_: context.clone_gl(),
            size: size,
            framebuffer: 0,
            color_attachment: None,
            stencil_renderbuffer: 0,
            depth_renderbuffer: 0,
            packed_depth_stencil_renderbuffer: 0,
            // samples: 0,
        };

        context.make_current()?;

        draw_buffer.init(context, color_attachment_type)?;

        debug_assert_eq!(draw_buffer.gl().check_frame_buffer_status(gl::FRAMEBUFFER),
                         gl::FRAMEBUFFER_COMPLETE);
        debug_assert_eq!(draw_buffer.gl().get_error(),
                         gl::NO_ERROR);

        Ok(draw_buffer)
    }

    #[inline(always)]
    pub fn get_framebuffer(&self) -> GLuint {
        self.framebuffer
    }

    #[inline(always)]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline(always)]
    // NOTE: We unwrap here because after creation the draw buffer
    // always have a color attachment
    pub fn color_attachment_type(&self) -> ColorAttachmentType {
        self.color_attachment.as_ref().unwrap().color_attachment_type()
    }

    pub fn get_bound_color_renderbuffer_id(&self) -> Option<GLuint> {
        match self.color_attachment.as_ref().unwrap() {
            &ColorAttachment::Renderbuffer(id) => Some(id),
            _ => None,
        }
    }

    pub fn get_bound_texture_id(&self) -> Option<GLuint> {
        match self.color_attachment.as_ref().unwrap() {
            &ColorAttachment::Renderbuffer(_) => None,
            &ColorAttachment::Texture(id) => Some(id),
            &ColorAttachment::IOSurface(id, _) => Some(id),
        }
    }

    pub fn get_bound_io_surface_id(&self) -> Option<IOSurfaceID> {
        match self.color_attachment.as_ref().unwrap() {
            &ColorAttachment::Renderbuffer(_) => None,
            &ColorAttachment::Texture(_) => None,
            &ColorAttachment::IOSurface(_, id) => Some(id),
        }
    }

    fn gl(&self) -> &dyn gl::Gl {
        &*self.gl_
    }


    fn init<T: NativeGLContextMethods>(&mut self,
                                       context: &GLContext<T>,
                                       color_attachment_type: ColorAttachmentType)
        -> Result<(), &'static str> {
        let attrs = context.borrow_attributes();
        let formats = context.borrow_formats();

        assert!(self.color_attachment.is_none(),
                "Would leak color attachment!");
        self.color_attachment = match color_attachment_type {
            ColorAttachmentType::Renderbuffer => {
                let color_renderbuffer =
                    create_renderbuffer(self.gl(), formats.color_renderbuffer, &self.size);
                debug_assert!(color_renderbuffer != 0);

                Some(ColorAttachment::Renderbuffer(color_renderbuffer))
            },

            // TODO(ecoal95): Allow more customization of textures
            ColorAttachmentType::Texture => {
                let texture = self.gl().gen_textures(1)[0];
                debug_assert!(texture != 0);

                self.gl().bind_texture(gl::TEXTURE_2D, texture);
                self.gl().tex_image_2d(
                    gl::TEXTURE_2D,
                    0,
                    formats.texture_internal as GLint,
                    self.size.width,
                    self.size.height,
                    0,
                    formats.texture,
                    formats.texture_type,
                    None
                );

                // Low filtering to allow rendering
                self.gl().tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
                self.gl().tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

                // TODO(emilio): Check if these two are neccessary, probably not
                self.gl().tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
                self.gl().tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

                self.gl().bind_texture(gl::TEXTURE_2D, 0);

                debug_assert_eq!(self.gl().get_error(), gl::NO_ERROR);

                Some(ColorAttachment::Texture(texture))
            },

            ColorAttachmentType::IOSurface => {
                let texture = self.gl().gen_textures(1)[0];
                debug_assert!(texture != 0);

                self.gl().bind_texture(gl::TEXTURE_RECTANGLE, texture);
                let io_surface = unsafe {
                    let props = CFDictionary::from_CFType_pairs(
                        &[
                            (CFString::wrap_under_get_rule(io_surface::kIOSurfaceWidth),CFNumber::from(self.size.width).as_CFType()),
                            (CFString::wrap_under_get_rule(io_surface::kIOSurfaceHeight),CFNumber::from(self.size.height).as_CFType()),
                            (CFString::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerElement),CFNumber::from(4).as_CFType()),
                            (CFString::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerRow),CFNumber::from(self.size.width * 4).as_CFType()),
                            (CFString::wrap_under_get_rule(io_surface::kIOSurfaceIsGlobal),CFBoolean::from(true).as_CFType()),
                        ]
                    );
                    io_surface::new(&props)
                };

                io_surface.bind_to_gl_texture(self.size.width, self.size.height);

                // Low filtering to allow rendering
                self.gl().tex_parameter_i(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
                self.gl().tex_parameter_i(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

                // TODO(emilio): Check if these two are neccessary, probably not
                self.gl().tex_parameter_i(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
                self.gl().tex_parameter_i(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

                self.gl().bind_texture(gl::TEXTURE_2D, 0);

                debug_assert_eq!(self.gl().get_error(), gl::NO_ERROR);

                Some(ColorAttachment::IOSurface(texture, io_surface.get_id()))
            },
        };

        // After this we check if we need stencil and depth buffers
        if attrs.depth && attrs.stencil && formats.packed_depth_stencil {
            self.packed_depth_stencil_renderbuffer = create_renderbuffer(self.gl(), gl::DEPTH24_STENCIL8, &self.size);
            debug_assert!(self.packed_depth_stencil_renderbuffer != 0);
        } else {
            if attrs.depth {
                self.depth_renderbuffer = create_renderbuffer(self.gl(), formats.depth, &self.size);
                debug_assert!(self.depth_renderbuffer != 0);
            }

            if attrs.stencil {
                self.stencil_renderbuffer = create_renderbuffer(self.gl(), formats.stencil, &self.size);
                debug_assert!(self.stencil_renderbuffer != 0);
            }
        }

        self.framebuffer = self.gl().gen_framebuffers(1)[0];
        debug_assert!(self.framebuffer != 0);

        // Finally we attach them to the framebuffer
        self.attach_to_framebuffer()
    }

    fn attach_to_framebuffer(&mut self) -> Result<(), &'static str> {
        self.gl().bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer);
        // NOTE: The assertion fails if the framebuffer is not bound
        debug_assert_eq!(self.gl().is_framebuffer(self.framebuffer), gl::TRUE);

        match *self.color_attachment.as_ref().unwrap() {
            ColorAttachment::Renderbuffer(color_renderbuffer) => {
                self.gl().framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                                  gl::COLOR_ATTACHMENT0,
                                                  gl::RENDERBUFFER,
                                                  color_renderbuffer);
                debug_assert_eq!(self.gl().is_renderbuffer(color_renderbuffer), gl::TRUE);
            },
            ColorAttachment::Texture(texture_id) => {
                self.gl().framebuffer_texture_2d(gl::FRAMEBUFFER,
                                                gl::COLOR_ATTACHMENT0,
                                                gl::TEXTURE_2D,
                                                texture_id, 0);
            },
            ColorAttachment::IOSurface(texture_id, _) => {
                self.gl().framebuffer_texture_2d(gl::FRAMEBUFFER,
                                                gl::COLOR_ATTACHMENT0,
                                                gl::TEXTURE_RECTANGLE,
                                                texture_id, 0);
            }
        }

        if self.packed_depth_stencil_renderbuffer != 0 {
            self.gl().framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                               gl::DEPTH_STENCIL_ATTACHMENT,
                                               gl::RENDERBUFFER,
                                               self.packed_depth_stencil_renderbuffer);
            debug_assert_eq!(self.gl().is_renderbuffer(self.packed_depth_stencil_renderbuffer), gl::TRUE);
        }

        if self.depth_renderbuffer != 0 {
            self.gl().framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                              gl::DEPTH_ATTACHMENT,
                                              gl::RENDERBUFFER,
                                              self.depth_renderbuffer);
            debug_assert_eq!(self.gl().is_renderbuffer(self.depth_renderbuffer), gl::TRUE);
        }

        if self.stencil_renderbuffer != 0 {
            self.gl().framebuffer_renderbuffer(gl::FRAMEBUFFER,
                                              gl::STENCIL_ATTACHMENT,
                                              gl::RENDERBUFFER,
                                              self.stencil_renderbuffer);
            debug_assert_eq!(self.gl().is_renderbuffer(self.stencil_renderbuffer), gl::TRUE);
        }

        Ok(())
    }
}

// NOTE: The initially associated GLContext MUST be the current gl context
// when drop is called. I know this is an important constraint.
// Right now there are no problems, if not, consider using a pointer to a
// parent with Rc<GLContext> and call make_current()
impl Drop for DrawBuffer {
    fn drop(&mut self) {
        if let Some(att) = self.color_attachment.take() {
            att.destroy(self.gl());
        }

        self.gl().delete_framebuffers(&[self.framebuffer]);

        // NOTE: Color renderbuffer is destroyed on drop of
        //   ColorAttachment
        self.gl().delete_renderbuffers(&[self.stencil_renderbuffer,
                                         self.depth_renderbuffer,
                                         self.packed_depth_stencil_renderbuffer]);
    }
}
