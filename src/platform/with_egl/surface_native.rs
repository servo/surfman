use crate::egl::types::{EGLint, EGLBoolean, EGLDisplay, EGLSurface, EGLConfig};
use crate::egl::types::{EGLContext, EGLNativeDisplayType};
use crate::egl;
use crate::gl_context::GLVersion;
use crate::gl_formats::Format;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl, GlType};
use std::borrow::BorrowMut;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

pub struct EGLDisplayWrapper(pub EGLDisplay);

unsafe impl Sync for EGLDisplayWrapper {}

lazy_static! {
    pub static ref DISPLAY: EGLDisplayWrapper = {
        unsafe {
            let display = egl::GetDisplay(egl::DEFAULT_DISPLAY as EGLNativeDisplayType);
            if display == egl::NO_DISPLAY as EGLDisplay {
                panic!("No EGL display found!");
            }

            if egl::Initialize(display, ptr::null_mut(), ptr::null_mut()) == 0 {
                panic!("Failed to initialize the EGL display!");
            }

            EGLDisplayWrapper(display)
        }
    };
}

pub struct EGLSurfaceWrapper(pub EGLSurface);

#[derive(Clone)]
pub struct NativeSurface {
    wrapper: Arc<EGLSurfaceWrapper>,
    config: EGLConfig,
    api_type: GlType,
    api_version: GLVersion,
    size: Size2D<i32>,
    format: Format,
}

#[derive(Debug)]
pub struct NativeSurfaceTexture {
    surface: NativeSurface,
    gl_texture: GLuint,
    phantom: PhantomData<*const ()>,
}

unsafe impl Send for EGLSurfaceWrapper {}

unsafe impl Send for NativeSurface {}

impl Drop for EGLSurfaceWrapper {
    fn drop(&mut self) {
        unsafe {
            egl::DestroySurface(DISPLAY.0, self.0);
        }
    }
}

impl Debug for NativeSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}, {:?}", self.size, self.format)
    }
}

impl NativeSurface {
    pub(crate) fn from_version_size_format(api_type: GlType,
                                           api_version: GLVersion,
                                           size: &Size2D<i32>,
                                           format: Format)
                                           -> NativeSurface {
        let renderable_type = get_pbuffer_renderable_type(api_type, api_version);

        // FIXME(pcwalton): Convert the formats to an appropriate set of EGL attributes!
        let pbuffer_attributes = [
            egl::SURFACE_TYPE as EGLint, egl::PBUFFER_BIT as EGLint,
            egl::RENDERABLE_TYPE as EGLint, renderable_type as EGLint,
            egl::BIND_TO_TEXTURE_RGBA as EGLint, 1 as EGLint,
            egl::RED_SIZE as EGLint, 8,
            egl::GREEN_SIZE as EGLint, 8,
            egl::BLUE_SIZE as EGLint, 8,
            egl::ALPHA_SIZE as EGLint, 0,
            egl::NONE as EGLint, 0,
            0, 0,
        ];

        unsafe {
            let (mut config, mut configs_found) = (ptr::null(), 0);
            if egl::ChooseConfig(DISPLAY.0,
                                 pbuffer_attributes.as_ptr(),
                                 &mut config,
                                 1,
                                 &mut configs_found) != egl::TRUE as u32 {
                panic!("Failed to choose an EGL configuration: {}!", egl::GetError())
            }

            if configs_found == 0 {
                panic!("No valid EGL configurations found!")
            }
            
            let attrs = [
                egl::WIDTH as EGLint, size.width as EGLint,
                egl::HEIGHT as EGLint, size.height as EGLint,
                egl::TEXTURE_FORMAT as EGLint, egl::TEXTURE_RGBA as EGLint,
                egl::TEXTURE_TARGET as EGLint, egl::TEXTURE_2D as EGLint,
                egl::NONE as EGLint, 0,
                0, 0, // see mod.rs
            ];

            let egl_surface = egl::CreatePbufferSurface(DISPLAY.0, config, attrs.as_ptr());
            if egl_surface == egl::NO_SURFACE as EGLSurface {
                panic!("Failed to create EGL surface!");
            }

            NativeSurface {
                wrapper: Arc::new(EGLSurfaceWrapper(egl_surface)),
                config,
                api_type,
                api_version,
                size: *size,
                format,
            }
        }
    }

    pub fn new(_: &dyn Gl,
               api_type: GlType,
               api_version: GLVersion,
               size: &Size2D<i32>,
               format: Format)
               -> NativeSurface {
        NativeSurface::from_version_size_format(api_type, api_version, size, format)
    }

    #[inline]
    pub(crate) fn egl_surface(&self) -> EGLSurface {
        self.wrapper.0
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn format(&self) -> Format {
        self.format
    }

    #[inline]
    pub(crate) fn config(&self) -> &EGLConfig {
        &self.config
    }

    #[inline]
    pub fn id(&self) -> u32 {
        self.egl_surface() as usize as u32
    }

    #[inline]
    pub(crate) fn api_type(&self) -> GlType {
        self.api_type
    }

    #[inline]
    pub(crate) fn api_version(&self) -> GLVersion {
        self.api_version
    }
}

impl NativeSurfaceTexture {
    pub fn new(gl: &dyn Gl, native_surface: NativeSurface) -> NativeSurfaceTexture {
        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_2D, texture);

        unsafe {
            if egl::BindTexImage(DISPLAY.0,
                                 native_surface.wrapper.0,
                                 egl::BACK_BUFFER as EGLint) == egl::FALSE {
                panic!("Failed to bind EGL texture surface: {}!", egl::GetError())
            }
        }

        // Low filtering to allow rendering
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

        // TODO(emilio): Check if these two are neccessary, probably not
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

        gl.bind_texture(gl::TEXTURE_2D, 0);

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
    pub fn gl_texture_target() -> GLenum {
        gl::TEXTURE_2D
    }

    #[inline]
    pub fn destroy(&mut self, gl: &dyn Gl) {
        unsafe {
            egl::ReleaseTexImage(DISPLAY.0, self.surface.wrapper.0, self.gl_texture as GLint);
        }

        gl.delete_textures(&[self.gl_texture]);
        self.gl_texture = 0;
    }
}

fn get_pbuffer_renderable_type(api_type: GlType, api_version: GLVersion) -> EGLint {
    match (api_type, api_version.major_version()) {
        (GlType::Gl, _) => egl::OPENGL_BIT as EGLint,
        (GlType::Gles, version) if version < 2 => egl::OPENGL_ES_BIT as EGLint,
        (GlType::Gles, 2) => egl::OPENGL_ES2_BIT as EGLint,
        (GlType::Gles, _) => egl::OPENGL_ES3_BIT as EGLint,
    }
}
