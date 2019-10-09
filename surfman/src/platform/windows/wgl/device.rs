// surfman/src/platform/windows/wgl/device.rs
//
//! An implementation of the GPU device for Windows using the WGL API.

use crate::Error;
use super::adapter::Adapter;
use super::context::WGL_EXTENSION_FUNCTIONS;

use winapi::um::d3d11::{D3D11CreateDevice, D3D11_SDK_VERSION, ID3D11Device};
use wio::com::ComPtr;

pub struct Device {
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d11_device_context: ComPtr<ID3D11DeviceContext>,
    pub(crate) gl_dx_interop_device: HANDLE,
}

impl Drop for Device {
    fn drop(&mut self) {
        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS.dx_interop_functions.as_ref().unwrap();
        unsafe {
            (dx_interop_functions.DXCloseDeviceNV)(self.gl_dx_interop_device);
        }
    }
}

impl Device {
    pub fn new(_: &Adapter) -> Result<Device, Error> {
        let dx_interop_functions = match WGL_EXTENSION_FUNCTIONS.dx_interop_functions {
            Some(ref dx_interop_functions) => dx_interop_functions,
            None => return Err(Error::RequiredExtensionUnavailable);
        };

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
            let d3d11_device = ComPtr::from_raw(d3d11_device);
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let gl_dx_interop_device =
                (dx_interop_functions.DXOpenDeviceNV)(d3d11_device.as_ptr());
            assert!(!gl_dx_interop_device.is_null());

            Ok(Device { d3d11_device, d3d11_device_context, gl_dx_interop_device })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
    }
}
