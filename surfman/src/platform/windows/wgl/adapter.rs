// surfman/src/platform/src/windows/wgl/adapter.rs
//
//! An adapter type for WGL.

use std::ptr;
use winapi::shared::ntdef::LPCSTR;
use winapi::um::libloaderapi;

static NVIDIA_GPU_SELECT_SYMBOL: &[u8] = b"NvOptimusEnablement\0";
static AMD_GPU_SELECT_SYMBOL: &[u8] = b"AmdPowerXpressRequestHighPerformance\0";

/// A no-op adapter.
#[derive(Clone, Debug)]
pub enum Adapter {
    HighPerformance,
    LowPower,
}

impl Adapter {
    pub(crate) fn set_exported_variables(&self) {
        unsafe {
            let current_module = libloaderapi::GetModuleHandleA(ptr::null());
            assert!(!current_module.is_null());
            let nvidia_gpu_select_variable: *mut i32 = libloaderapi::GetProcAddress(
                current_module,
                NVIDIA_GPU_SELECT_SYMBOL.as_ptr() as LPCSTR) as *mut i32;
            let amd_gpu_select_variable: *mut i32 = libloaderapi::GetProcAddress(
                current_module,
                AMD_GPU_SELECT_SYMBOL.as_ptr() as LPCSTR) as *mut i32;
            if nvidia_gpu_select_variable.is_null() || amd_gpu_select_variable.is_null() {
                println!("surfman: Could not find the NVIDIA and/or AMD GPU selection symbols. \
                       Your application may end up using the wrong GPU (discrete vs. \
                       integrated). To fix this issue, ensure that you are using the MSVC \
                       version of Rust and invoke the `declare_surfman!()` macro at the root of \
                       your crate.");
                warn!("surfman: Could not find the NVIDIA and/or AMD GPU selection symbols. \
                       Your application may end up using the wrong GPU (discrete vs. \
                       integrated). To fix this issue, ensure that you are using the MSVC \
                       version of Rust and invoke the `declare_surfman!()` macro at the root of \
                       your crate.");
                return;
            }
            let value = match *self {
                Adapter::HighPerformance => 1,
                Adapter::LowPower => 0,
            };
            *nvidia_gpu_select_variable = value;
            *amd_gpu_select_variable = value;
        }
    }
}

