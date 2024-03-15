// surfman/surfman/src/platform/windows/angle/device.rs
//
//! A thread-local handle to the device.

use super::connection::Connection;
use crate::egl;
use crate::egl::types::{EGLAttrib, EGLDeviceEXT, EGLDisplay, EGLint};
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::ffi::EGL_DEVICE_EXT;
use crate::platform::generic::egl::ffi::{EGL_D3D11_DEVICE_ANGLE, EGL_EXTENSION_FUNCTIONS};
use crate::platform::generic::egl::ffi::{EGL_NO_DEVICE_EXT, EGL_PLATFORM_DEVICE_EXT};
use crate::{Error, GLApi};

use std::cell::{RefCell, RefMut};
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use winapi::shared::dxgi::{self, IDXGIAdapter, IDXGIDevice, IDXGIFactory1};
use winapi::shared::minwindef::UINT;
use winapi::shared::winerror::{self, S_OK};
use winapi::um::d3d11::{D3D11CreateDevice, ID3D11Device, D3D11_SDK_VERSION};
use winapi::um::d3dcommon::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_9_3};
use winapi::Interface;
use wio::com::ComPtr;

thread_local! {
    static DXGI_FACTORY: RefCell<Option<ComPtr<IDXGIFactory1>>> = RefCell::new(None);
}

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone)]
pub struct Adapter {
    pub(crate) dxgi_adapter: ComPtr<IDXGIAdapter>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

unsafe impl Send for Adapter {}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) egl_display: EGLDisplay,
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
    pub(crate) display_is_owned: bool,
}

pub(crate) enum VendorPreference {
    None,
    Prefer(UINT),
    Avoid(UINT),
}

/// Wraps a Direct3D 11 device and its associated EGL display.
#[derive(Clone)]
pub struct NativeDevice {
    /// The ANGLE EGL display.
    pub egl_display: EGLDisplay,
    /// The Direct3D 11 device that the ANGLE EGL display was created with.
    pub d3d11_device: *mut ID3D11Device,
    /// The Direct3D driver type that the device was created with.
    pub d3d_driver_type: D3D_DRIVER_TYPE,
}

impl Adapter {
    pub(crate) fn new(
        d3d_driver_type: D3D_DRIVER_TYPE,
        vendor_preference: VendorPreference,
    ) -> Result<Adapter, Error> {
        unsafe {
            let dxgi_factory = DXGI_FACTORY.with(|dxgi_factory_slot| {
                let mut dxgi_factory_slot: RefMut<Option<ComPtr<IDXGIFactory1>>> =
                    dxgi_factory_slot.borrow_mut();
                if dxgi_factory_slot.is_none() {
                    let mut dxgi_factory: *mut IDXGIFactory1 = ptr::null_mut();
                    let result = dxgi::CreateDXGIFactory1(
                        &IDXGIFactory1::uuidof(),
                        &mut dxgi_factory as *mut *mut IDXGIFactory1 as *mut *mut c_void,
                    );
                    if !winerror::SUCCEEDED(result) {
                        return Err(Error::Failed);
                    }
                    assert!(!dxgi_factory.is_null());
                    *dxgi_factory_slot = Some(ComPtr::from_raw(dxgi_factory));
                }
                Ok((*dxgi_factory_slot).clone().unwrap())
            })?;

            // Find the first adapter that matches the vendor preference.
            let mut adapter_index = 0;
            loop {
                let mut dxgi_adapter_1 = ptr::null_mut();
                let result = (*dxgi_factory).EnumAdapters1(adapter_index, &mut dxgi_adapter_1);
                if !winerror::SUCCEEDED(result) {
                    break;
                }
                assert!(!dxgi_adapter_1.is_null());
                let dxgi_adapter_1 = ComPtr::from_raw(dxgi_adapter_1);

                let mut adapter_desc = mem::zeroed();
                let result = (*dxgi_adapter_1).GetDesc1(&mut adapter_desc);
                assert_eq!(result, S_OK);

                let choose_this = match vendor_preference {
                    VendorPreference::Prefer(vendor_id) => vendor_id == adapter_desc.VendorId,
                    VendorPreference::Avoid(vendor_id) => vendor_id != adapter_desc.VendorId,
                    VendorPreference::None => true,
                };
                if choose_this {
                    let mut dxgi_adapter: *mut IDXGIAdapter = ptr::null_mut();
                    let result = (*dxgi_adapter_1).QueryInterface(
                        &IDXGIAdapter::uuidof(),
                        &mut dxgi_adapter as *mut *mut IDXGIAdapter as *mut *mut c_void,
                    );
                    assert_eq!(result, S_OK);
                    let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

                    return Ok(Adapter {
                        dxgi_adapter,
                        d3d_driver_type,
                    });
                }

                adapter_index += 1;
            }

            // Fallback: Go with the first adapter.
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
                &mut dxgi_adapter as *mut *mut IDXGIAdapter as *mut *mut c_void,
            );
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Ok(Adapter {
                dxgi_adapter,
                d3d_driver_type,
            })
        }
    }

    /// Create an Adapter instance wrapping an existing DXGI adapter.
    pub fn from_dxgi_adapter(adapter: ComPtr<IDXGIAdapter>) -> Adapter {
        Adapter {
            dxgi_adapter: adapter,
            d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
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
            let result = D3D11CreateDevice(
                adapter.dxgi_adapter.as_raw(),
                d3d_driver_type,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                0,
                D3D11_SDK_VERSION,
                &mut d3d11_device,
                &mut d3d11_feature_level,
                &mut d3d11_device_context,
            );
            if !winerror::SUCCEEDED(result) {
                return Err(Error::DeviceOpenFailed);
            }
            debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
            let d3d11_device = ComPtr::from_raw(d3d11_device);

            let eglCreateDeviceANGLE = EGL_EXTENSION_FUNCTIONS
                .CreateDeviceANGLE
                .expect("Where's the `EGL_ANGLE_device_creation` extension?");
            let egl_device = eglCreateDeviceANGLE(
                EGL_D3D11_DEVICE_ANGLE as EGLint,
                d3d11_device.as_raw() as *mut c_void,
                ptr::null_mut(),
            );
            assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

            EGL_FUNCTIONS.with(|egl| {
                let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
                let egl_display = egl.GetPlatformDisplay(
                    EGL_PLATFORM_DEVICE_EXT,
                    egl_device as *mut c_void,
                    &attribs[0],
                );
                assert_ne!(egl_display, egl::NO_DISPLAY);

                // I don't think this should ever fail.
                let (mut major_version, mut minor_version) = (0, 0);
                let result = egl.Initialize(egl_display, &mut major_version, &mut minor_version);
                assert_ne!(result, egl::FALSE);

                Ok(Device {
                    egl_display,
                    d3d11_device,
                    d3d_driver_type,
                    display_is_owned: true,
                })
            })
        }
    }

    pub(crate) fn from_native_device(native_device: NativeDevice) -> Result<Device, Error> {
        unsafe {
            (*native_device.d3d11_device).AddRef();
            Ok(Device {
                egl_display: native_device.egl_display,
                d3d11_device: ComPtr::from_raw(native_device.d3d11_device),
                d3d_driver_type: native_device.d3d_driver_type,
                display_is_owned: false,
            })
        }
    }

    #[allow(non_snake_case)]
    pub(crate) fn from_egl_display(egl_display: EGLDisplay) -> Result<Device, Error> {
        let eglQueryDisplayAttribEXT = EGL_EXTENSION_FUNCTIONS
            .QueryDisplayAttribEXT
            .expect("Where's the `EGL_EXT_device_query` extension?");
        let eglQueryDeviceAttribEXT = EGL_EXTENSION_FUNCTIONS
            .QueryDeviceAttribEXT
            .expect("Where's the `EGL_EXT_device_query` extension?");
        let mut angle_device: EGLAttrib = 0;
        let result = eglQueryDisplayAttribEXT(
            egl_display,
            EGL_DEVICE_EXT as EGLint,
            &mut angle_device as *mut EGLAttrib,
        );
        if result == egl::FALSE {
            return Err(Error::DeviceOpenFailed);
        }
        let mut device: EGLAttrib = 0;
        let result = eglQueryDeviceAttribEXT(
            angle_device as EGLDeviceEXT,
            EGL_D3D11_DEVICE_ANGLE as EGLint,
            &mut device as *mut EGLAttrib,
        );
        if result == egl::FALSE {
            return Err(Error::DeviceOpenFailed);
        }
        let d3d11_device = device as *mut ID3D11Device;

        unsafe {
            (*d3d11_device).AddRef();
            Ok(Device {
                egl_display: egl_display,
                d3d11_device: ComPtr::from_raw(d3d11_device),
                d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
                display_is_owned: false,
            })
        }
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    pub fn adapter(&self) -> Adapter {
        unsafe {
            let mut dxgi_device: *mut IDXGIDevice = ptr::null_mut();
            let result = (*self.d3d11_device).QueryInterface(
                &IDXGIDevice::uuidof(),
                &mut dxgi_device as *mut *mut IDXGIDevice as *mut *mut c_void,
            );
            assert!(winerror::SUCCEEDED(result));
            let dxgi_device = ComPtr::from_raw(dxgi_device);

            let mut dxgi_adapter = ptr::null_mut();
            let result = (*dxgi_device).GetAdapter(&mut dxgi_adapter);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Adapter {
                dxgi_adapter,
                d3d_driver_type: self.d3d_driver_type,
            }
        }
    }

    /// Returns the underlying native device type.
    ///
    /// The reference count on the underlying Direct3D device is increased before returning it.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice {
            egl_display: self.egl_display,
            d3d11_device: self.d3d11_device.clone().into_raw(),
            d3d_driver_type: self.d3d_driver_type,
        }
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            if self.display_is_owned {
                EGL_FUNCTIONS.with(|egl| {
                    let result = egl.Terminate(self.egl_display);
                    assert_ne!(result, egl::FALSE);
                    self.egl_display = egl::NO_DISPLAY;
                })
            }
        }
    }
}
