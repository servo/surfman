// surfman/surfman/src/platform/windows/angle/adapter.rs
//
//! A wrapper for DXGI adapters and Direct3D 11 drivers.

use crate::Error;

use std::cell::{RefCell, RefMut};
use std::os::raw::c_void;
use std::ptr;
use winapi::Interface;
use winapi::shared::dxgi::{self, IDXGIAdapter, IDXGIFactory1};
use winapi::shared::winerror;
use winapi::um::d3dcommon::D3D_DRIVER_TYPE;
use wio::com::ComPtr;

thread_local! {
    static DXGI_FACTORY: RefCell<Option<ComPtr<IDXGIFactory1>>> = RefCell::new(None);
}

#[derive(Clone)]
pub struct Adapter {
    pub(crate) dxgi_adapter: ComPtr<IDXGIAdapter>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

unsafe impl Send for Adapter {}

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
