#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(unused)]

use std::ffi::c_void;

/// From `eglext.h` on OpenHarmony.
pub(crate) const EGL_NATIVE_BUFFER_OHOS: u32 = 0x34E1;

use crate::egl::types::EGLClientBuffer;

#[repr(C)]
pub struct NativeWindow {
    _unused: [u8; 0],
}

pub type OHNativeWindow = NativeWindow;

#[repr(transparent)]
pub(crate) struct NativeWindowOperation(core::ffi::c_int);

impl NativeWindowOperation {
    pub const GET_BUFFER_GEOMETRY: Self = Self(1);
}

/// According to the [native window guidelines], users need to link against
/// both the NDK and `native_window`.
/// [native window guidelines]: <https://gitee.com/openharmony/docs/blob/master/en/application-dev/graphics/native-window-guidelines.md>
#[link(name = "native_window")]
#[link(name = "ace_ndk.z")]
extern "C" {
    /// Sets or obtains the attributes of a native window
    ///
    /// Can be used to query information like height and width.
    /// See the official [Documentation] for detailed usage information.
    ///
    /// # Safety
    ///
    ///  - The `window` handle must be valid.
    ///  - The variable arguments which must be passed to this function vary depending on the
    ///    value of `code`.
    ///  - For `NativeWindowOperation::GET_BUFFER_GEOMETRY` the function takes two output
    ///    i32 pointers, `height: *mut i32` and `width: *mut i32` which are passed as variadic
    ///    arguments.
    ///
    ///
    /// [Documentation]: <https://gitee.com/openharmony/docs/blob/master/en/application-dev/reference/apis-arkgraphics2d/_native_window.md>
    pub(crate) fn OH_NativeWindow_NativeWindowHandleOpt(
        window: *mut OHNativeWindow,
        code: NativeWindowOperation,
        ...
    ) -> i32;
}

#[link(name = "EGL")]
extern "C" {
    /// Get the native Client buffer
    ///
    /// The extension function `eglGetNativeClientBufferANDROID` is available starting with OpenHarmony 5.0.
    /// Its availability is documented here: https://docs.openharmony.cn/pages/v5.0/en/application-dev/reference/native-lib/egl-symbol.md
    /// However it is not available in `EGL_EXTENSION_FUNCTIONS`, since `eglGetProcAddress()` does not find
    /// the function and returns NULL.
    pub(crate) fn eglGetNativeClientBufferANDROID(buffer: *const c_void) -> EGLClientBuffer;
}

// Bindings to `native_buffer` components we use. Official Documentation:
// https://docs.openharmony.cn/pages/v5.0/en/application-dev/graphics/native-buffer-guidelines.md

#[repr(C)]
pub struct OH_NativeBuffer {
    _opaque: [u8; 0],
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct OH_NativeBuffer_Config {
    /// Width in pixels
    pub width: i32,
    /// Height in pixels
    pub height: i32,
    /// One of PixelFormat
    pub format: OH_NativeBuffer_Format,
    /// Combination of buffer usage
    pub usage: OH_NativeBuffer_Usage,
    /// the stride of memory
    pub stride: i32,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct OH_NativeBuffer_Format(core::ffi::c_int);

impl OH_NativeBuffer_Format {
    /// RGBA8888 format
    pub const RGBA_8888: OH_NativeBuffer_Format = OH_NativeBuffer_Format(12);
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Copy, Clone)]
    pub struct OH_NativeBuffer_Usage: core::ffi::c_int {
        /// CPU read buffer
        const CPU_READ = 1;
        /// CPU write memory
        const CPU_WRITE = 1 << 1;
        /// Direct memory access (DMA) buffer
        const MEM_DMA = 1 << 3;
        /// For GPU write case
        const HW_RENDER = 1 << 8;
        /// For GPU read case
        const HW_TEXTURE = 1 << 9;
        /// Often be mapped for direct CPU reads
        const CPU_READ_OFTEN = 1 << 16;
        /// 512 bytes alignment
        const ALIGNMENT_512 = 1 << 18;
    }
}

#[link(name = "native_buffer")]
extern "C" {
    /// Allocate an `OH_NativeBuffer`` that matches the passed config.
    ///
    /// A new `OH_NativeBuffer` instance is created each time this function is called.
    /// NULL is returned if allocation fails.
    pub fn OH_NativeBuffer_Alloc(config: *const OH_NativeBuffer_Config) -> *mut OH_NativeBuffer;
    /// Decreases the reference count of a OH_NativeBuffer and, when the reference count reaches 0,
    /// destroys this OH_NativeBuffer.
    ///
    /// Since API-9
    pub fn OH_NativeBuffer_Unreference(buffer: *mut OH_NativeBuffer) -> i32;
}
