use crate::egl::types::{EGLAttrib, EGLBoolean, EGLConfig, EGLContext, EGLDeviceEXT, EGLDisplay};
use crate::egl::types::{EGLSurface, EGLenum, EGLint};
use crate::egl;
use crate::gl_context::GLVersion;
use crate::gl_formats::Format;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl, GlType};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::thread;
use winapi::Interface;
use winapi::shared::dxgi::{IDXGIAdapter, IDXGIDevice};
use winapi::shared::winerror;
use winapi::um::d3d11::{D3D11CreateDevice, D3D11_SDK_VERSION};
use winapi::um::d3d11::{ID3D11Device, ID3D11DeviceContext};
use winapi::um::d3dcommon::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_REFERENCE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_9_3};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winnt::HANDLE;
use wio::com::ComPtr;

const BYTES_PER_PIXEL: i32 = 4;

const EGL_NO_DEVICE_EXT: EGLDeviceEXT = 0 as EGLDeviceEXT;
const EGL_PLATFORM_DEVICE_EXT: EGLenum = 0x313f;
const EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE: EGLenum = 0x3200;
const EGL_D3D11_DEVICE_ANGLE: EGLint = 0x33a1;

struct SurfaceHandle {
    share_handle: HANDLE,
    size: Size2D<i32>,
    api_type: GlType,
    api_version: GLVersion,
    locked: AtomicBool,
}

// NB: Be careful cloning this; the `egl_surface` and `egl_config` members are equivalent to
// unsafe pointers.
#[derive(Clone)]
struct AngleSurface {
    egl_surface: EGLSurface,
    egl_config: EGLConfig,
}

struct SurfaceEntry {
    handle: Weak<SurfaceHandle>,
    angle_surface: AngleSurface,
}

#[derive(Clone)]
pub struct Surface {
    handle: Arc<SurfaceHandle>,
    format: Format,
}

#[derive(Debug)]
pub struct SurfaceTexture {
    surface: Surface,
    gl_texture: GLuint,
    phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:?})", self.id())
    }
}

impl Surface {
    pub(crate) fn from_version_size_format(api_type: GlType,
                                           api_version: GLVersion,
                                           size: &Size2D<i32>,
                                           format: Format)
                                           -> Surface {
        Display::with(|display| {
            unsafe {
                let egl_config = display.api_to_config(api_type, api_version);

                let attributes = [
                    egl::WIDTH as EGLint,  size.width as EGLint,
                    egl::HEIGHT as EGLint, size.height as EGLint,
                    egl::NONE as EGLint,   0,
                    0,                     0,
                ];

                let egl_surface = egl::CreatePbufferSurface(display.egl_display,
                                                            egl_config,
                                                            attributes.as_ptr());
                debug_assert_ne!(egl_surface, egl::NO_SURFACE);

                let mut share_handle = INVALID_HANDLE_VALUE;
                let result =
                    eglQuerySurfacePointerANGLE(display.egl_display,
                                                egl_surface,
                                                EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE as EGLint,
                                                &mut share_handle);
                debug_assert_ne!(result, egl::FALSE);
                debug_assert_ne!(share_handle, INVALID_HANDLE_VALUE);

                let handle = Arc::new(SurfaceHandle {
                    share_handle,
                    api_type,
                    api_version,
                    size: *size,
                    locked: AtomicBool::new(false),
                });

                display.surfaces.push(SurfaceEntry {
                    handle: Arc::downgrade(&handle),
                    angle_surface: AngleSurface { egl_surface, egl_config },
                });

                Surface { handle, format }
            }
        })
    }

    pub fn new(_: &dyn Gl,
               api_type: GlType,
               api_version: GLVersion,
               size: &Size2D<i32>,
               format: Format)
               -> Surface {
        Surface::from_version_size_format(api_type, api_version, size, format)
    }

    #[inline]
    pub(crate) fn egl_surface(&self) -> EGLSurface {
        Display::with(|display| display.get_angle_surface(&self.handle).egl_surface)
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.handle.size
    }

    #[inline]
    pub fn format(&self) -> Format {
        self.format
    }

    #[inline]
    pub(crate) fn config(&self) -> EGLConfig {
        Display::with(|display| display.get_angle_surface(&self.handle).egl_config)
    }

    #[inline]
    pub fn id(&self) -> u32 {
        &*self.handle as *const _ as usize as u32
    }

    #[inline]
    pub(crate) fn api_type(&self) -> GlType {
        self.handle.api_type
    }

    #[inline]
    pub(crate) fn api_version(&self) -> GLVersion {
        self.handle.api_version
    }

    #[inline]
    pub(crate) fn lock_surface(&self) {
        if self.handle.locked.swap(true, Ordering::SeqCst) {
            panic!("Attempted to lock an already-locked surface!")
        }
    }

    #[inline]
    pub(crate) fn unlock_surface(&self) {
        if !self.handle.locked.swap(false, Ordering::SeqCst) {
            panic!("Attempted to unlock an unlocked surface!")
        }
    }
}

impl SurfaceTexture {
    pub fn new(gl: &dyn Gl, native_surface: Surface) -> SurfaceTexture {
        native_surface.lock_surface();

        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_2D, texture);

        Display::with(|display| {
            unsafe {
                let egl_surface = display.get_angle_surface(&native_surface.handle).egl_surface;
                if egl::BindTexImage(display.egl_display,
                                     egl_surface,
                                     egl::BACK_BUFFER as GLint) == egl::FALSE {
                    panic!("Failed to bind EGL texture surface: {:x}!", egl::GetError())
                }
            }
        });

        // Low filtering to allow rendering
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);

        // TODO(emilio): Check if these two are neccessary, probably not
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

        gl.bind_texture(gl::TEXTURE_2D, 0);

        debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

        SurfaceTexture { surface: native_surface, gl_texture: texture, phantom: PhantomData }
    }

    #[inline]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[inline]
    pub fn into_surface(mut self, gl: &dyn Gl) -> Surface {
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
        Display::with(|display| {
            unsafe {
                egl::ReleaseTexImage(display.egl_display,
                                     display.get_angle_surface(&self.surface.handle).egl_surface,
                                     self.gl_texture as GLint);
            }
        });

        gl.delete_textures(&[self.gl_texture]);
        self.gl_texture = 0;

        self.surface.unlock_surface();
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
