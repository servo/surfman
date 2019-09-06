//! A wrapper for DXGI adapters and Direct3D 11 drivers.

use crate::Error;
use std::cell::RefCell;
use winapi::shared::dxgi::{self, IDXGIAdapter, IDXGIDevice};
use wio::com::ComPtr;

thread_local! {
    static DXGI_FACTORY: RefCell<Option<ComPtr<IDXGIFactory1>>> = RefCell::new(None);
}

#[derive(Clone)]
pub struct Adapter {
    pub(crate) dxgi_adapter: ComPtr<IDXGIDevice>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

impl Adapter {
    pub fn default() -> Result<Adapter, Error> {
        unsafe {
            let dxgi_factory = DXGI_FACTORY.with(|dxgi_factory| {
                let mut dxgi_factory_slot = dxgi_factory_slot.borrow_mut();
                if dxgi_factory_slot.is_none() {
                    let mut dxgi_factory = ptr::null_mut();
                    let result = dxgi::CreateDXGIFactory1(IDXGIFactory1::uuidof(),
                                                          &mut *dxgi_factory);
                    if !winerror::SUCCEEDED(result) {
                        return Err(Error::Failed);
                    }
                    assert!(!dxgi_factory.is_null());
                    *dxgi_factory_slot = ComPtr::from_raw(dxgi_factory);
                }
                (*dxgi_factory_slot).cloned().unwrap()
            });

            let mut dxgi_adapter = ptr::null_mut();
            let result = (*dxgi_factory).EnumAdapters1(0, &mut dxgi_adapter);
            if !winerror::SUCCEEDED(result) {
                return Err(Error::NoAdapterFound);
            }
            assert!(!dxgi_adapter.is_null());
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Ok(Adapter {
                dxgi_adapter,
                d3d_driver_type: D3D_DRIVER_TYPE_HARDWARE,
            })
        }
    }
}
