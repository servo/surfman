use crate::egl::types::{EGLint, EGLBoolean, EGLDisplay, EGLSurface, EGLConfig};
use crate::egl::types::{EGLContext, EGLNativeDisplayType};
use crate::egl;
use crate::gl_context::GLVersion;
use crate::gl_formats::Format;
use euclid::default::Size2D;
use gleam::gl::{self, GLenum, GLint, GLuint, Gl, GlType};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::ptr;
use std::sync::{Arc, Weak};
use std::thread;
use weak_table::PtrWeakKeyHashMap;

const BYTES_PER_PIXEL: i32 = 4;

pub struct Display {
    d3d11_device: ComPtr<ID3D11Device>,
    d3d11_device_context: ComPtr<ID3D11DeviceContext>,
    egl_device: EGLDeviceEXT,
    pub egl_display: EGLDisplay,
    surfaces: Vec<SurfaceEntry>,
}

thread_local! {
    static DISPLAY: RefCell<Option<Display>> = RefCell::new(None);
}

impl Display {
    fn with<F, R>(&'static self, callback: F) -> R where F: FnOnce(&mut Display) -> R {
        DISPLAY.with(|display| {
            let mut display = display.borrow_mut();
            if display.is_none() {
                unsafe {
                    let mut d3d11_device = ptr::null_mut();
                    let mut d3d11_feature_level = 0;
                    let mut d3d11_device_context = ptr::null_mut();
                    let result = D3D11CreateDevice(ptr::null_mut(),
                                                   D3D_DRIVER_TYPE_HARDWARE,
                                                   0,
                                                   0,
                                                   ptr::null_mut(),
                                                   0,
                                                   D3D11_SDK_VERSION,
                                                   &mut d3d11_device,
                                                   &mut d3d11_feature_level,
                                                   &mut d3d11_device_context);
                    assert!(SUCCEEDED(result));
                    debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
                    let d3d11_device = ComPtr::new(d3d11_device);
                    let d3d11_device_context = ComPtr::new(d3d11_device_context);

                    let egl_device = eglCreateDeviceANGLE(EGL_D3D11_DEVICE_ANGLE,
                                                          d3d11_device.as_raw()
                                                          ptr::null_mut());
                    assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

                    let attribs = [EGL_NONE, EGL_NONE];
                    let egl_display = eglGetPlatformDisplay(EGL_PLATFORM_DEVICE_EXT,
                                                            egl_device,
                                                            &attribs[0]);
                    assert_ne!(egl_display, EGL_NO_DISPLAY);

                    *display = Some(Display {
                        d3d11_device,
                        d3d11_device_context,
                        egl_device,
                        egl_display,
                        surfaces: vec![],
                    })
                }
            }

            callback(display.as_mut().unwrap())
        })
    }

    fn sweep_dead_surfaces(&mut self) {
        self.surfaces.retain(|surface| {
            let dead = surface.handle.upgrade().is_none();
            if dead {
                let ok = egl::DestroySurface(self.egl_display, surface.angle_surface);
                debug_assert_ne!(ok, EGL_FALSE);
            }
            dead
        })
    }

    fn api_to_config(&self, api_type: GlType, api_version: GLVersion) -> EGLConfig {
        let renderable_type = get_pbuffer_renderable_type(api_type, api_version);

        // FIXME(pcwalton): Convert the GL formats to an appropriate set of EGL attributes!
        let pbuffer_attributes = [
            egl::SURFACE_TYPE as EGLint,         egl::PBUFFER_BIT as EGLint,
            egl::RENDERABLE_TYPE as EGLint,      renderable_type as EGLint,
            egl::BIND_TO_TEXTURE_RGBA as EGLint, 1 as EGLint,
            egl::TEXTURE_TARGET as EGLint,       gl::TEXTURE_2D as EGLint,
            egl::RED_SIZE as EGLint,             8,
            egl::GREEN_SIZE as EGLint,           8,
            egl::BLUE_SIZE as EGLint,            8,
            egl::ALPHA_SIZE as EGLint,           0,
            egl::NONE as EGLint,                 0,
            0,                                   0,
        ];

        unsafe {
            let (mut config, mut configs_found) = (ptr::null(), 0);
            if egl::ChooseConfig(DISPLAY.0,
                                 pbuffer_attributes.as_ptr(),
                                 &mut config,
                                 1,
                                 &mut configs_found) != egl::TRUE as u32 {
                panic!("Failed to choose an EGL configuration!")
            }

            if configs_found == 0 {
                panic!("No valid EGL configurations found!")
            }

            config
        }
    }

    // TODO(pcwalton): This is O(n) in the number of surfaces. Might be a problem with many
    // surfaces.
    fn get_surface(&mut self, query: &Arc<SurfaceHandle>) -> EGLSurface {
        // Find an existing surface if we have one.
        for surface in &self.surfaces {
            if let Some(handle) = surface.handle.upgrade() {
                if (&**query).ptr_eq(&*handle) {
                    return surface.angle_surface
                }
            }
        }

        // We don't have an EGL surface yet. Create one from the D3D handle.
        let angle_config = self.api_to_config(query.api_type, query.api_version);
        let attributes = [
            egl::TEXTURE_FORMAT, egl::TEXTURE_RGBA,
            egl::TEXTURE_TARGET, egl::TEXTURE_2D,
            egl::NONE,           0,
            0,                   0,
        ];
        let angle_surface =
            egl::CreatePBufferFromClientBuffer(self.egl_display,
                                               egl::D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                                               query.0,
                                               angle_config,
                                               attributes.as_ptr());
        assert_ne!(angle_surface, egl::NO_SURFACE);

        // Cache our new surface and return it.
        self.surfaces.push(SurfaceEntry {
            handle: (*query).clone().downgrade(),
            angle_surface,
            angle_config,
        });

        angle_surface
    }
}

struct SurfaceHandle {
    share_handle: HANDLE,
    api_type: GlType,
    api_version: GLVersion,
}

struct SurfaceEntry {
    handle: Weak<SurfaceHandle>,
    angle_surface: EGLSurface,
    angle_config: EGLConfig,
}

#[derive(Clone)]
pub struct NativeSurface {
    handle: Arc<SurfaceHandle>,
    size: Size2D<i32>,
    format: Format,
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
        write!(f, "{:?}, {:?}", self.size, self.format)
    }
}

impl NativeSurface {
    pub(crate) fn from_version_size_format(api_type: GlType,
                                           api_version: GLVersion,
                                           size: &Size2D<i32>,
                                           format: Format)
                                           -> NativeSurface {
        DISPLAY.with(|display| {
            let angle_config = display.api_to_config(api_type, api_version);

            let attributes = [
                egl::WIDTH as EGLint,  size.width as EGLint,
                egl::HEIGHT as EGLint, size.height as EGLint,
                egl::NONE as EGLint,   0,
                0,                     0,
            ];

            let angle_surface = egl::CreatePbufferSurface(display.egl_display,
                                                          angle_config,
                                                          attributes.as_ptr());
            debug_assert_ne!(angle_surface, egl::NO_SURFACE)

            let mut share_handle = INVALID_HANDLE_VALUE;
            let result =
                egl::QuerySurfaceAttribPointerANGLE(display.egl_display,
                                                    angle_surface,
                                                    egl::D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                                                    &mut share_handle);
            debug_assert_ne!(result, egl::FALSE);
            debug_assert_ne!(share_handle, INVALID_HANDLE_VALUE);

            let handle = Arc::new(SurfaceHandle { share_handle, api_type, api_version });
            display.surfaces.push(SurfaceEntry {
                handle: handle.clone().downgrade(),
                angle_surface,
                angle_config,
            });

            NativeSurface { handle, size: *size, format }
        })
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
        &self.handle.config
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
}

impl NativeSurfaceTexture {
    pub fn new(gl: &dyn Gl, native_surface: NativeSurface) -> NativeSurfaceTexture {
        let texture = gl.gen_textures(1)[0];
        debug_assert!(texture != 0);

        gl.bind_texture(gl::TEXTURE_2D, texture);

        DISPLAY.with(|display| {
            unsafe {
                if egl::BindTexImage(display.egl_display,
                                     native_surface.wrapper.0,
                                     texture as GLint) == egl::FALSE {
                    panic!("Failed to bind EGL texture surface!")
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
