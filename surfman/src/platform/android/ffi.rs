// surfman/surfman/src/platform/android/ffi.rs

use std::os::raw::c_int;

pub(crate) const AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM: u32 = 1;

pub(crate) const AHARDWAREBUFFER_USAGE_CPU_READ_NEVER: u64 = 0;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER: u64 = 0 << 4;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE: u64 = 1 << 8;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER: u64 = 1 << 9;

#[repr(C)]
pub struct AHardwareBuffer {
    opaque: i32,
}

#[repr(C)]
pub(crate) struct AHardwareBuffer_Desc {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layers: u32,
    pub(crate) format: u32,
    pub(crate) usage: u64,
    pub(crate) stride: u32,
    pub(crate) rfu0: u32,
    pub(crate) rfu1: u64,
}

#[repr(C)]
pub struct ANativeWindow {
    opaque: i32,
}

#[link(name = "android")]
extern "C" {
    pub(crate) fn AHardwareBuffer_allocate(
        desc: *const AHardwareBuffer_Desc,
        outBuffer: *mut *mut AHardwareBuffer,
    ) -> c_int;
    pub(crate) fn AHardwareBuffer_release(buffer: *mut AHardwareBuffer);

    pub(crate) fn ANativeWindow_getWidth(window: *mut ANativeWindow) -> i32;
    pub(crate) fn ANativeWindow_getHeight(window: *mut ANativeWindow) -> i32;
}
