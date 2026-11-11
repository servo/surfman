//! The OpenHarmony parts of hardware buffer surfaces, which use `OH_NativeBuffer` and
//! `OHNativeWindow`.

use super::super::ohos_ffi::{
    eglGetNativeClientBufferANDROID, NativeWindowOperation, OHNativeWindow, OH_NativeBuffer,
    OH_NativeBuffer_Alloc, OH_NativeBuffer_Config, OH_NativeBuffer_Format,
    OH_NativeBuffer_Unreference, OH_NativeBuffer_Usage, OH_NativeWindow_NativeWindowHandleOpt,
    EGL_NATIVE_BUFFER_OHOS,
};
use super::{NativeBufferSurface, NativeWindowSurface};
use crate::base::egl::ffi::EGLClientBuffer;
use crate::egl::types::EGLenum;
use crate::{Error, WindowingApiError};

use euclid::default::Size2D;
use raw_window_handle::RawWindowHandle;
use std::fmt::{self, Debug, Formatter};

pub(crate) type NativeBuffer = OH_NativeBuffer;
pub(crate) type NativeWindow = OHNativeWindow;

/// A surface displayed in an OpenHarmony native window.
pub struct OhosWindowSurface(pub(crate) NativeWindowSurface);

/// An offscreen surface backed by an OpenHarmony `OH_NativeBuffer`.
pub struct OhosBufferSurface(pub(crate) NativeBufferSurface);

impl Debug for OhosWindowSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "OhosWindowSurface({:x})", self.0.id().0)
    }
}

impl Debug for OhosBufferSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "OhosBufferSurface({:x})", self.0.id().0)
    }
}

/// Returns the native window of a window handle, if it is one of this platform.
pub(crate) fn native_window(window_handle: RawWindowHandle) -> Result<*mut NativeWindow, Error> {
    match window_handle {
        RawWindowHandle::OhosNdk(handle) => Ok(handle.native_window.as_ptr().cast()),
        _ => Err(Error::IncompatibleSurfaceType),
    }
}

pub(crate) fn allocate_native_buffer(size: &Size2D<i32>) -> Result<*mut NativeBuffer, Error> {
    let config = OH_NativeBuffer_Config {
        width: size.width,
        height: size.height,
        format: OH_NativeBuffer_Format::RGBA_8888,
        usage: OH_NativeBuffer_Usage::HW_RENDER | OH_NativeBuffer_Usage::HW_TEXTURE,
        stride: 10, // used same magic number as android. I have no idea
    };
    let buffer = unsafe { OH_NativeBuffer_Alloc(&config) };
    if buffer.is_null() {
        return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
    }
    Ok(buffer)
}

/// # Safety
///
/// `buffer` must be a buffer returned by [`allocate_native_buffer`] that is not released yet.
pub(crate) unsafe fn release_native_buffer(buffer: *mut NativeBuffer) {
    let result = unsafe { OH_NativeBuffer_Unreference(buffer) };
    assert_eq!(result, 0, "OH_NativeBuffer_Unreference failed");
}

/// # Safety
///
/// `window` must be a valid native window.
pub(crate) unsafe fn native_window_size(window: *mut NativeWindow) -> Size2D<i32> {
    let mut height: i32 = 0;
    let mut width: i32 = 0;
    // Safety: `OH_NativeWindow_NativeWindowHandleOpt` takes two output i32 pointers as
    // variable arguments when called with `GET_BUFFER_GEOMETRY`.
    let result = unsafe {
        OH_NativeWindow_NativeWindowHandleOpt(
            window,
            NativeWindowOperation::GET_BUFFER_GEOMETRY,
            &mut height as *mut i32,
            &mut width as *mut i32,
        )
    };
    assert_eq!(result, 0, "Failed to determine size of native window");
    Size2D::new(width, height)
}

/// Returns the EGL client buffer of a native buffer, and the target to make an EGL image of it
/// with.
///
/// # Safety
///
/// `buffer` must be a buffer returned by [`allocate_native_buffer`] that is not released yet.
pub(crate) unsafe fn native_client_buffer(buffer: *mut NativeBuffer) -> (EGLClientBuffer, EGLenum) {
    let client_buffer = unsafe { eglGetNativeClientBufferANDROID(buffer.cast_const().cast()) };
    (client_buffer.cast_mut().cast(), EGL_NATIVE_BUFFER_OHOS)
}
