#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(unused)]

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
    /// [Documentation]: <https://gitee.com/openharmony/docs/blob/master/en/application-dev/reference/apis-arkgraphics2d/_native_window.md>
    pub(crate) fn OH_NativeWindow_NativeWindowHandleOpt(
        window: *mut OHNativeWindow,
        code: NativeWindowOperation,
        ...
    ) -> i32;
}
