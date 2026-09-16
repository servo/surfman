/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A hardware display adapter for ANGLE on Windows systems.

use super::device::VendorPreference;
use crate::Error;

use std::cell::{RefCell, RefMut};
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use winapi::shared::dxgi::{self, IDXGIAdapter, IDXGIFactory1};
use winapi::shared::winerror::{self, S_OK};
use winapi::um::d3dcommon::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN};
use winapi::Interface;
use wio::com::ComPtr;

thread_local! {
    static DXGI_FACTORY: RefCell<Option<ComPtr<IDXGIFactory1>>> = RefCell::new(None);
}

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Debug, Clone)]
pub struct AngleAdapter {
    pub(crate) dxgi_adapter: ComPtr<IDXGIAdapter>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

unsafe impl Send for AngleAdapter {}

impl AngleAdapter {
    pub(crate) fn new(
        d3d_driver_type: D3D_DRIVER_TYPE,
        vendor_preference: VendorPreference,
    ) -> Result<Self, Error> {
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

                    return Ok(Self {
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

            Ok(Self {
                dxgi_adapter,
                d3d_driver_type,
            })
        }
    }

    /// Create an Adapter instance wrapping an existing DXGI adapter.
    pub fn from_dxgi_adapter(adapter: ComPtr<IDXGIAdapter>) -> Self {
        Self {
            dxgi_adapter: adapter,
            d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
        }
    }
}
