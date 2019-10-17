// surfman/src/platform/android/ffi.rs

use std::os::raw::{c_int, c_void};

pub(crate) const AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM:     u32 = 0;
pub(crate) const AHARDWAREBUFFER_FORMAT_R8G8B8X8_UNORM:     u32 = 2;
pub(crate) const AHARDWAREBUFFER_FORMAT_R8G8B8_UNORM:       u32 = 3;
pub(crate) const AHARDWAREBUFFER_FORMAT_R5G6B5_UNORM:       u32 = 4;
pub(crate) const AHARDWAREBUFFER_FORMAT_R16G16B16A16_FLOAT: u32 = 0x16;
pub(crate) const AHARDWAREBUFFER_FORMAT_R10G10B10A2_UNORM:  u32 = 0x2b;
pub(crate) const AHARDWAREBUFFER_FORMAT_BLOB:               u32 = 0x21;
pub(crate) const AHARDWAREBUFFER_FORMAT_D16_UNORM:          u32 = 0x30;
pub(crate) const AHARDWAREBUFFER_FORMAT_D24_UNORM:          u32 = 0x31;
pub(crate) const AHARDWAREBUFFER_FORMAT_D24_UNORM_S8_UINT:  u32 = 0x32;
pub(crate) const AHARDWAREBUFFER_FORMAT_D32_FLOAT:          u32 = 0x33;
pub(crate) const AHARDWAREBUFFER_FORMAT_D32_FLOAT_S8_UINT:  u32 = 0x34;
pub(crate) const AHARDWAREBUFFER_FORMAT_S8_UINT:            u32 = 0x35;
pub(crate) const AHARDWAREBUFFER_FORMAT_Y8Cb8Cr8_420:       u32 = 0x23;

pub(crate) const AHARDWAREBUFFER_USAGE_CPU_READ_NEVER:      u64 = 0;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_READ_RARELY:     u64 = 2;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_READ_OFTEN:      u64 = 3;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_READ_MASK:       u64 = 0xf;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER:     u64 = 0 << 4;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_WRITE_RARELY:    u64 = 2 << 4;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_WRITE_OFTEN:     u64 = 3 << 4;
pub(crate) const AHARDWAREBUFFER_USAGE_CPU_WRITE_MASK:      u64 = 0xf << 4;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE:   u64 = 1 << 8;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER:     u64 = 1 << 9;
pub(crate) const AHARDWAREBUFFER_USAGE_COMPOSER_OVERLAY:    u64 = 1 << 11;
pub(crate) const AHARDWAREBUFFER_USAGE_PROTECTED_CONTENT:   u64 = 1 << 14;
pub(crate) const AHARDWAREBUFFER_USAGE_VIDEO_ENCODE:        u64 = 1 << 16;
pub(crate) const AHARDWAREBUFFER_USAGE_SENSOR_DIRECT_DATA:  u64 = 1 << 23;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_DATA_BUFFER:     u64 = 1 << 24;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_CUBE_MAP:        u64 = 1 << 25;
pub(crate) const AHARDWAREBUFFER_USAGE_GPU_MIPMAP_COMPLETE: u64 = 1 << 26;

pub(crate) const AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT: u64 =
    AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER;

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
extern {
    pub(crate) fn AHardwareBuffer_allocate(desc: *const AHardwareBuffer_Desc,
                                           outBuffer: *mut *mut AHardwareBuffer)
                                           -> c_int;
    pub(crate) fn AHardwareBuffer_acquire(buffer: *mut AHardwareBuffer);
    pub(crate) fn AHardwareBuffer_release(buffer: *mut AHardwareBuffer);
    pub(crate) fn AHardwareBuffer_describe(buffer: *const AHardwareBuffer,
                                           outDesc: *mut AHardwareBuffer_Desc);
    

    pub(crate) fn ANativeWindow_getWidth(window: *mut ANativeWindow) -> i32;
    pub(crate) fn ANativeWindow_getHeight(window: *mut ANativeWindow) -> i32;
}
