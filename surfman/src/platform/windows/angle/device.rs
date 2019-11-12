// surfman/surfman/src/platform/windows/angle/device.rs
//
//! A thread-local handle to the device.

use crate::egl::types::{EGLAttrib, EGLint};
use crate::egl;
use crate::platform::generic::egl::device::{EGL_FUNCTIONS, NativeDisplay, OwnedEGLDisplay};
use crate::platform::generic::egl::ffi::{EGL_D3D11_DEVICE_ANGLE, EGL_EXTENSION_FUNCTIONS};
use crate::platform::generic::egl::ffi::{EGL_NO_DEVICE_EXT, EGL_PLATFORM_DEVICE_EXT};
use crate::{Error, GLApi};
use super::connection::Connection;

use std::cell::{RefCell, RefMut};
use std::os::raw::c_void;
use std::ptr;
use winapi::Interface;
use winapi::shared::dxgi::{self, IDXGIAdapter, IDXGIDevice, IDXGIFactory1};
use winapi::shared::winerror;
use winapi::um::d3d11::{D3D11CreateDevice, D3D11_SDK_VERSION, ID3D11Device};
use winapi::um::d3dcommon::{D3D_DRIVER_TYPE, D3D_FEATURE_LEVEL_9_3};
use wio::com::ComPtr;

thread_local! {
    static DXGI_FACTORY: RefCell<Option<ComPtr<IDXGIFactory1>>> = RefCell::new(None);
}

/// A wrapper for DXGI adapters and Direct3D 11 drivers.
#[derive(Clone)]
pub struct Adapter {
    pub(crate) dxgi_adapter: ComPtr<IDXGIAdapter>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

unsafe impl Send for Adapter {}

pub struct Device {
    pub(crate) native_display: Box<dyn NativeDisplay>,
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

impl Adapter {
    pub(crate) fn from_driver_type(d3d_driver_type: D3D_DRIVER_TYPE) -> Result<Adapter, Error> {
        unsafe {
            let dxgi_factory = DXGI_FACTORY.with(|dxgi_factory_slot| {
                let mut dxgi_factory_slot: RefMut<Option<ComPtr<IDXGIFactory1>>> =
                        dxgi_factory_slot.borrow_mut();
                if dxgi_factory_slot.is_none() {
                    let mut dxgi_factory: *mut IDXGIFactory1 = ptr::null_mut();
                    let result = dxgi::CreateDXGIFactory1(
                        &IDXGIFactory1::uuidof(),
                        &mut dxgi_factory as *mut *mut IDXGIFactory1 as *mut *mut c_void);
                    if !winerror::SUCCEEDED(result) {
                        return Err(Error::Failed);
                    }
                    assert!(!dxgi_factory.is_null());
                    *dxgi_factory_slot = Some(ComPtr::from_raw(dxgi_factory));
                }
                Ok((*dxgi_factory_slot).clone().unwrap())
            })?;

            let mut dxgi_adapter_1 = ptr::null_mut();
            let result = (*dxgi_factory).EnumAdapters1(0, &mut dxgi_adapter_1);
            if !winerror::SUCCEEDED(result) {
                return Err(Error::NoAdapterFound);
            }
            assert!(!dxgi_adapter_1.is_null());
            let dxgi_adapter_1 = ComPtr::from_raw(dxgi_adapter_1);

            let mut dxgi_adapter: *mut IDXGIAdapter = ptr::null_mut();
            let result = (*dxgi_adapter_1).QueryInterface(
                &IDXGIAdapter::uuidof(),
                &mut dxgi_adapter as *mut *mut IDXGIAdapter as *mut *mut c_void);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Ok(Adapter { dxgi_adapter, d3d_driver_type })
        }
    }
}

impl Device {
    #[allow(non_snake_case)]
    pub(crate) fn new(adapter: &Adapter) -> Result<Device, Error> {
        let d3d_driver_type = adapter.d3d_driver_type;
        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let mut d3d11_device_context = ptr::null_mut();
            let result = D3D11CreateDevice(adapter.dxgi_adapter.as_raw(),
                                           d3d_driver_type,
                                           ptr::null_mut(),
                                           0,
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

            let eglCreateDeviceANGLE =
                EGL_EXTENSION_FUNCTIONS.CreateDeviceANGLE
                                       .expect("Where's the `EGL_ANGLE_device_creation` \
                                                extension?");
            let egl_device = eglCreateDeviceANGLE(EGL_D3D11_DEVICE_ANGLE as EGLint,
                                                  d3d11_device.as_raw() as *mut c_void,
                                                  ptr::null_mut());
            assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

            EGL_FUNCTIONS.with(|egl| {
                let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
                let egl_display = egl.GetPlatformDisplay(EGL_PLATFORM_DEVICE_EXT,
                                                         egl_device as *mut c_void,
                                                         &attribs[0]);
                assert_ne!(egl_display, egl::NO_DISPLAY);
                let native_display = Box::new(OwnedEGLDisplay { egl_display });

                // I don't think this should ever fail.
                let (mut major_version, mut minor_version) = (0, 0);
                let result = egl.Initialize(native_display.egl_display(),
                                            &mut major_version,
                                            &mut minor_version);
                assert_ne!(result, egl::FALSE);

                Ok(Device { native_display, d3d11_device, d3d_driver_type })
            })
        }
    }

    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    pub fn d3d11_device(&self) -> ComPtr<ID3D11Device> {
        self.d3d11_device.clone()
    }

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

    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }
}
