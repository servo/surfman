//! A thread-local handle to the device.

use crate::egl::types::{EGLAttrib, EGLBoolean, EGLConfig, EGLContext, EGLDeviceEXT, EGLDisplay};
use crate::egl::types::{EGLSurface, EGLenum, EGLint};
use crate::{Error, egl};
use super::adapter::Adapter;
use super::context::ContextID;

use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr;
use winapi::Interface;
use winapi::shared::dxgi::IDXGIDevice;
use winapi::shared::winerror;
use winapi::um::d3d11::{D3D11CreateDevice, D3D11_CREATE_DEVICE_DEBUG, D3D11_SDK_VERSION, ID3D11Device};
use winapi::um::d3dcommon::{D3D_DRIVER_TYPE, D3D_FEATURE_LEVEL_9_3};
use wio::com::ComPtr;

pub(crate) const EGL_D3D11_DEVICE_ANGLE: EGLint = 0x33a1;
pub(crate) const EGL_NO_DEVICE_EXT: EGLDeviceEXT = 0 as EGLDeviceEXT;

const EGL_PLATFORM_DEVICE_EXT: EGLenum = 0x313f;

pub(crate) struct EGLExtensionFunctions {
    CreateDeviceANGLE: extern "C" fn(device_type: EGLint,
                                     native_device: *mut c_void,
                                     attrib_list: *const EGLAttrib)
                                     -> EGLDeviceEXT,
    pub(crate) QueryDeviceAttribEXT: extern "C" fn(device: EGLDeviceEXT,
                                                   attribute: EGLint,
                                                   value: *mut EGLAttrib)
                                                   -> EGLBoolean,
    pub(crate) QueryDisplayAttribEXT: extern "C" fn(dpy: EGLDisplay,
                                                    attribute: EGLint,
                                                    value: *mut EGLAttrib)
                                                    -> EGLBoolean,
    pub(crate) QuerySurfacePointerANGLE: extern "C" fn(dpy: EGLDisplay,
                                                       surface: EGLSurface,
                                                       attribute: EGLint,
                                                       value: *mut *mut c_void)
                                                       -> EGLBoolean,
}

lazy_static! {
    pub(crate) static ref EGL_EXTENSION_FUNCTIONS: EGLExtensionFunctions = {
        unsafe {
            EGLExtensionFunctions {
                CreateDeviceANGLE:
                    mem::transmute(lookup_egl_extension(b"eglCreateDeviceANGLE\0")),
                QueryDeviceAttribEXT:
                    mem::transmute(lookup_egl_extension(b"eglQueryDeviceAttribEXT\0")),
                QueryDisplayAttribEXT:
                    mem::transmute(lookup_egl_extension(b"eglQueryDisplayAttribEXT\0")),
                QuerySurfacePointerANGLE:
                    mem::transmute(lookup_egl_extension(b"eglQuerySurfacePointerANGLE\0")),
            }
        }
    };
}

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
    pub(crate) egl_device: EGLDeviceEXT,
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

pub(crate) trait NativeDisplay {
    fn egl_display(&self) -> EGLDisplay;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self);
}

impl Device {
    #[inline]
    pub fn new(adapter: &Adapter) -> Result<Device, Error> {
        let d3d_driver_type = adapter.d3d_driver_type;
        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let mut d3d11_device_context = ptr::null_mut();
            let result = D3D11CreateDevice(adapter.dxgi_adapter.as_raw(),
                                           d3d_driver_type,
                                           ptr::null_mut(),
                                           D3D11_CREATE_DEVICE_DEBUG,
                                           ptr::null_mut(),
                                           0,
                                           D3D11_SDK_VERSION,
                                           &mut d3d11_device,
                                           &mut d3d11_feature_level,
                                           &mut d3d11_device_context);
            if !winerror::SUCCEEDED(result) {
                return Err(Error::DeviceOpenFailed);
            }
            debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
            let d3d11_device = ComPtr::from_raw(d3d11_device);
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let egl_device = (EGL_EXTENSION_FUNCTIONS.CreateDeviceANGLE)(
                EGL_D3D11_DEVICE_ANGLE,
                d3d11_device.as_raw() as *mut c_void,
                ptr::null_mut());
            assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

            let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
            let egl_display = egl::GetPlatformDisplay(EGL_PLATFORM_DEVICE_EXT,
                                                      egl_device as *mut c_void,
                                                      &attribs[0]);
            assert_ne!(egl_display, egl::NO_DISPLAY);
            let native_display = Box::new(OwnedEGLDisplay { egl_display });

            // I don't think this should ever fail.
            let (mut major_version, mut minor_version) = (0, 0);
            let result = egl::Initialize(native_display.egl_display(),
                                         &mut major_version,
                                         &mut minor_version);
            assert_ne!(result, egl::FALSE);

            Ok(Device {
                native_display,
                egl_device,
                d3d11_device,
                d3d_driver_type,
            })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        unsafe {
            let mut dxgi_device: *mut IDXGIDevice = ptr::null_mut();
            let result = (*self.d3d11_device).QueryInterface(
                &IDXGIDevice::uuidof(),
                &mut dxgi_device as *mut *mut IDXGIDevice as *mut *mut c_void);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_device = ComPtr::from_raw(dxgi_device);

            let mut dxgi_adapter = ptr::null_mut();
            let result = (*dxgi_device).GetAdapter(&mut dxgi_adapter);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Adapter { dxgi_adapter, d3d_driver_type: self.d3d_driver_type }
        }
    }
}

unsafe fn lookup_egl_extension(name: &'static [u8]) -> *mut c_void {
    let f = egl::GetProcAddress(&name[0] as *const u8 as *const c_char);
    assert_ne!(f as usize, 0);
    f as *mut c_void
}

pub(crate) struct OwnedEGLDisplay {
    pub(crate) egl_display: EGLDisplay,
}

impl NativeDisplay for OwnedEGLDisplay {
    #[inline]
    fn egl_display(&self) -> EGLDisplay {
        debug_assert!(!self.is_destroyed());
        self.egl_display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_display == egl::NO_DISPLAY
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        let result = egl::Terminate(self.egl_display);
        assert_ne!(result, egl::FALSE);
        self.egl_display = egl::NO_DISPLAY;
    }
}

pub(crate) struct UnsafeEGLDisplayRef {
    egl_display: EGLDisplay,
}

impl NativeDisplay for UnsafeEGLDisplayRef {
    #[inline]
    fn egl_display(&self) -> EGLDisplay {
        debug_assert!(!self.is_destroyed());
        self.egl_display
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_display == egl::NO_DISPLAY
    }

    unsafe fn destroy(&mut self) {
        assert!(!self.is_destroyed());
        self.egl_display = egl::NO_DISPLAY;
    }
}
