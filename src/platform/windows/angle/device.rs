// surfman/surfman/src/platform/windows/angle/device.rs
//
//! A thread-local handle to the device.

use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_9_3,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIDevice, IDXGIFactory1,
};

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

thread_local! {
    static DXGI_FACTORY: RefCell<Option<IDXGIFactory1>> = RefCell::new(None);
}

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone)]
pub struct Adapter {
    pub(crate) dxgi_adapter: IDXGIAdapter,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

unsafe impl Send for Adapter {}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) egl_display: EGLDisplay,
    pub(crate) d3d11_device: ID3D11Device,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
    pub(crate) display_is_owned: bool,
}

pub(crate) enum VendorPreference {
    None,
    Prefer(u32),
    Avoid(u32),
}

/// Wraps a Direct3D 11 device and its associated EGL display.
#[derive(Clone)]
pub struct NativeDevice {
    /// The ANGLE EGL display.
    pub egl_display: EGLDisplay,
    /// The Direct3D 11 device that the ANGLE EGL display was created with.
    pub d3d11_device: ID3D11Device,
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
                let result = CreateDXGIFactory1::<IDXGIFactory1>();
                if result.is_err() {
                    return Err(Error::Failed);
                }
                Ok(result.unwrap())
            })?;

            // Find the first adapter that matches the vendor preference.
            let mut adapter_index = 0;
            loop {
                let result = (*dxgi_factory).EnumAdapters(adapter_index);
                if result.is_err() {
                    break;
                }
                let dxgi_adapter_1 = result.unwrap();

                let result = dxgi_adapter_1.GetDesc();
                assert!(result.is_ok());
                let adapter_desc = result.unwrap();
                let choose_this = match vendor_preference {
                    VendorPreference::Prefer(vendor_id) => vendor_id == adapter_desc.VendorId,
                    VendorPreference::Avoid(vendor_id) => vendor_id != adapter_desc.VendorId,
                    VendorPreference::None => true,
                };
                if choose_this {
                    return Ok(Adapter {
                        dxgi_adapter: dxgi_adapter_1,
                        d3d_driver_type,
                    });
                }

                adapter_index += 1;
            }

            // Fallback: Go with the first adapter.
            let result = (*dxgi_factory).EnumAdapters(0);
            if result.is_err() {
                return Err(Error::NoAdapterFound);
            }
            let dxgi_adapter_1 = result.unwrap();

            Ok(Adapter {
                dxgi_adapter: dxgi_adapter_1,
                d3d_driver_type,
            })
        }
    }

    /// Create an Adapter instance wrapping an existing DXGI adapter.
    pub fn from_dxgi_adapter(adapter: IDXGIAdapter) -> Adapter {
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
            let mut d3d11_device = Default::default();
            let d3d11_feature_level = ptr::null_mut();
            let mut d3d11_device_context = Default::default();
            let d3d11_adapter: IDXGIAdapter = if d3d_driver_type == D3D_DRIVER_TYPE_WARP {
                IDXGIAdapter::from_raw(ptr::null_mut())
            } else {
                adapter.dxgi_adapter.clone()
            };
            let result = D3D11CreateDevice(
                &d3d11_adapter,
                d3d_driver_type,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d11_device),
                Some(d3d11_feature_level),
                Some(&mut d3d11_device_context),
            );

            if result.is_err() {
                return Err(Error::DeviceOpenFailed);
            }
            debug_assert!((*d3d11_feature_level).0 >= D3D_FEATURE_LEVEL_9_3.0);

            let eglCreateDeviceANGLE = EGL_EXTENSION_FUNCTIONS
                .CreateDeviceANGLE
                .expect("Where's the `EGL_ANGLE_device_creation` extension?");
            assert!(d3d11_device.is_some());
            let d3d11_device = d3d11_device.unwrap();
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
        Ok(Device {
            egl_display: native_device.egl_display,
            d3d11_device: native_device.d3d11_device,
            d3d_driver_type: native_device.d3d_driver_type,
            display_is_owned: false,
        })
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
        unsafe {
            let d3d11_device = ID3D11Device::from_raw(device as *mut c_void);
            Ok(Device {
                egl_display: egl_display,
                d3d11_device: d3d11_device,
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
            let result = (*self.d3d11_device).cast::<IDXGIDevice>();
            assert!(result.is_ok());
            let dxgi_device = result.unwrap();

            let result = dxgi_device.GetAdapter();
            assert!(result.is_ok());
            let dxgi_adapter = result.unwrap();

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
            d3d11_device: self.d3d11_device.clone(),
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
