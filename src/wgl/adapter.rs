/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A hardware display adapter for WGL on Windows systems.

use crate::{AdapterPreferences, PowerPreference, RenderingPreference};
use log::warn;
use std::ffi::CStr;
use winapi::um::libloaderapi;

static NVIDIA_GPU_SELECT_SYMBOL: &CStr = c"NvOptimusEnablement";
static AMD_GPU_SELECT_SYMBOL: &CStr = c"AmdPowerXpressRequestHighPerformance";

/// An implementation of [`crate::Adapter`] for WGL (Windows) platforms.
#[derive(Clone, Debug)]
pub struct WglAdapter(AdapterPreferences);

impl WglAdapter {
    pub(crate) fn new(preferences: AdapterPreferences) -> Self {
        Self(preferences)
    }

    pub(crate) fn set_exported_variables(&self) {
        unsafe {
            let current_module = libloaderapi::GetModuleHandleA(std::ptr::null());
            assert!(!current_module.is_null());
            let nvidia_gpu_select_variable: *mut i32 =
                libloaderapi::GetProcAddress(current_module, NVIDIA_GPU_SELECT_SYMBOL.as_ptr())
                    as *mut i32;
            let amd_gpu_select_variable: *mut i32 =
                libloaderapi::GetProcAddress(current_module, AMD_GPU_SELECT_SYMBOL.as_ptr())
                    as *mut i32;
            if nvidia_gpu_select_variable.is_null() || amd_gpu_select_variable.is_null() {
                println!(
                    "surfman: Could not find the NVIDIA and/or AMD GPU selection symbols. \
                       Your application may end up using the wrong GPU (discrete vs. \
                       integrated). To fix this issue, ensure that you are using the MSVC \
                       version of Rust and invoke the `declare_surfman!()` macro at the root of \
                       your crate."
                );
                warn!(
                    "surfman: Could not find the NVIDIA and/or AMD GPU selection symbols. \
                       Your application may end up using the wrong GPU (discrete vs. \
                       integrated). To fix this issue, ensure that you are using the MSVC \
                       version of Rust and invoke the `declare_surfman!()` macro at the root of \
                       your crate."
                );
                return;
            }

            let value = match (self.0.power, self.0.rendering) {
                (PowerPreference::Low, _) | (_, RenderingPreference::Software) => 0,
                (PowerPreference::High, _) => 1,
            };

            *nvidia_gpu_select_variable = value;
            *amd_gpu_select_variable = value;
        }
    }
}
