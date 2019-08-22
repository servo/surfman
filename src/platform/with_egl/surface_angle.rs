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

pub struct Display {
    d3d11_device: ComPtr<ID3D11Device>,
    d3d11_device_context: ComPtr<ID3D11DeviceContext>,
    egl_device: EGLDeviceEXT,
    pub egl_display: EGLDisplay,
    surfaces: Vec<SurfaceEntry>,
}

lazy_static! {
    static ref eglCreateDeviceANGLE: extern "C" fn(device_type: EGLint,
                                                   native_device: *mut c_void,
                                                   attrib_list: *const EGLAttrib)
                                                   -> EGLDeviceEXT = {
        unsafe {
            static NAME: &'static [u8] = b"eglCreateDeviceANGLE\0";
            let f = egl::GetProcAddress(&NAME[0] as *const u8 as *const c_char);
            assert_ne!(f as usize, 0);
            mem::transmute(f)
        }
    };
    static ref eglQuerySurfacePointerANGLE: extern "C" fn(dpy: EGLDisplay,
                                                          surface: EGLSurface,
                                                          attribute: EGLint,
                                                          value: *mut *mut c_void)
                                                          -> EGLBoolean = {
        unsafe {
            static NAME: &'static [u8] = b"eglQuerySurfacePointerANGLE\0";
            let f = egl::GetProcAddress(&NAME[0] as *const u8 as *const c_char);
            assert_ne!(f as usize, 0);
            mem::transmute(f)
        }
    };
}

thread_local! {
    pub static DISPLAY: RefCell<Option<Display>> = RefCell::new(None);
}

impl Display {
    pub(crate) fn with<F, R>(callback: F) -> R where F: FnOnce(&mut Display) -> R {
        DISPLAY.with(|display| {
            let mut display = display.borrow_mut();
            if display.is_none() {
                unsafe {
                    let mut d3d11_device = ptr::null_mut();
                    let mut d3d11_feature_level = 0;
                    let mut d3d11_device_context = ptr::null_mut();
                    let result = D3D11CreateDevice(ptr::null_mut(),
                                                   D3D_DRIVER_TYPE_HARDWARE,
                                                   ptr::null_mut(),
                                                   0,
                                                   ptr::null_mut(),
                                                   0,
                                                   D3D11_SDK_VERSION,
                                                   &mut d3d11_device,
                                                   &mut d3d11_feature_level,
                                                   &mut d3d11_device_context);
                    assert!(winerror::SUCCEEDED(result));
                    debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
                    let d3d11_device = ComPtr::from_raw(d3d11_device);
                    let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

                    let mut dxgi_device: *mut IDXGIDevice = ptr::null_mut();
                    let result = (*d3d11_device).QueryInterface(
                        &IDXGIDevice::uuidof(),
                        &mut dxgi_device as *mut *mut IDXGIDevice as *mut *mut c_void);
                    assert!(winerror::SUCCEEDED(result));
                    let dxgi_device = ComPtr::from_raw(dxgi_device);

                    let mut dxgi_adapter = ptr::null_mut();
                    let result = (*dxgi_device).GetAdapter(&mut dxgi_adapter);
                    assert!(winerror::SUCCEEDED(result));
                    let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

                    let mut desc = mem::zeroed();
                    let result = (*dxgi_adapter).GetDesc(&mut desc);
                    assert!(winerror::SUCCEEDED(result));

                    println!("Adapter name: {}", String::from_utf16_lossy(&desc.Description));

                    let egl_device = (*eglCreateDeviceANGLE)(EGL_D3D11_DEVICE_ANGLE,
                                                             d3d11_device.as_raw() as *mut c_void,
                                                             ptr::null_mut());
                    assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

                    let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
                    let egl_display = egl::GetPlatformDisplay(EGL_PLATFORM_DEVICE_EXT,
                                                              egl_device as *mut c_void,
                                                              &attribs[0]);
                    assert_ne!(egl_display, egl::NO_DISPLAY);

                    let (mut major_version, mut minor_version) = (0, 0);
                    let result = egl::Initialize(egl_display,
                                                 &mut major_version,
                                                 &mut minor_version);
                    assert_ne!(result, egl::FALSE);

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
        let egl_display = self.egl_display;
        self.surfaces.retain(|surface| {
            let dead = surface.handle.upgrade().is_none();
            if dead {
                unsafe {
                    let ok = egl::DestroySurface(egl_display, surface.angle_surface.egl_surface);
                    debug_assert_ne!(ok, egl::FALSE);
                }
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
            egl::RED_SIZE as EGLint,             8,
            egl::GREEN_SIZE as EGLint,           8,
            egl::BLUE_SIZE as EGLint,            8,
            egl::ALPHA_SIZE as EGLint,           0,
            egl::NONE as EGLint,                 0,
            0,                                   0,
        ];

        unsafe {
            let (mut config, mut configs_found) = (ptr::null(), 0);
            if egl::ChooseConfig(self.egl_display,
                                 pbuffer_attributes.as_ptr(),
                                 &mut config,
                                 1,
                                 &mut configs_found) != egl::TRUE as u32 {
                panic!("Failed to choose an EGL configuration: {:x}!",
                       egl::GetError())
            }

            if configs_found == 0 {
                panic!("No valid EGL configurations found!")
            }

            config
        }
    }

    // TODO(pcwalton): This is O(n) in the number of surfaces. Might be a problem with many
    // surfaces.
    fn get_angle_surface(&mut self, query: &Arc<SurfaceHandle>) -> AngleSurface {
        // Find an existing surface if we have one.
        for surface in &self.surfaces {
            if let Some(handle) = surface.handle.upgrade() {
                if ptr::eq(&**query, &*handle) {
                    return surface.angle_surface.clone();
                }
            }
        }

        // We don't have an EGL surface yet. Create one from the D3D handle.
        let egl_config = self.api_to_config(query.api_type, query.api_version);
        let attributes = [
            egl::WIDTH as EGLint,          query.size.width,
            egl::HEIGHT as EGLint,         query.size.height,
            egl::TEXTURE_FORMAT as EGLint, egl::TEXTURE_RGBA as EGLint,
            egl::TEXTURE_TARGET as EGLint, egl::TEXTURE_2D as EGLint,
            egl::NONE as EGLint,           egl::NONE as EGLint,
            0,                             0,
        ];
        let egl_surface = unsafe {
            egl::CreatePbufferFromClientBuffer(self.egl_display,
                                               EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                                               query.share_handle,
                                               egl_config,
                                               attributes.as_ptr())
        };
        if egl_surface == egl::NO_SURFACE {
            unsafe {
                panic!("eglCreatePbufferFromClientBuffer failed: {:x}", egl::GetError());
            }
        }

        // Cache our new surface and return it.
        let angle_surface = AngleSurface { egl_surface, egl_config };
        self.surfaces.push(SurfaceEntry { handle: Arc::downgrade(query), angle_surface });
        self.surfaces.last().unwrap().angle_surface.clone()
    }
}

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
pub struct NativeSurface {
    handle: Arc<SurfaceHandle>,
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
        write!(f, "Surface({:?})", self.id())
    }
}

impl NativeSurface {
    pub(crate) fn from_version_size_format(api_type: GlType,
                                           api_version: GLVersion,
                                           size: &Size2D<i32>,
                                           format: Format)
                                           -> NativeSurface {
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

                NativeSurface { handle, format }
            }
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

impl NativeSurfaceTexture {
    pub fn new(gl: &dyn Gl, native_surface: NativeSurface) -> NativeSurfaceTexture {
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
