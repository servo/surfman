//! The Android parts of hardware buffer surfaces, which use `AHardwareBuffer` and
//! `ANativeWindow`.

use super::super::android_ffi::{
    AHardwareBuffer, AHardwareBuffer_Desc, AHardwareBuffer_acquire, AHardwareBuffer_allocate,
    AHardwareBuffer_describe, AHardwareBuffer_release, ANativeWindow, ANativeWindow_getHeight,
    ANativeWindow_getWidth, AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
    AHARDWAREBUFFER_USAGE_CPU_READ_NEVER, AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER,
    AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER, AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE,
};
use super::{NativeBufferSurface, NativeWindowSurface};
use crate::base::egl::ffi::{EGLClientBuffer, EGL_EXTENSION_FUNCTIONS, EGL_NATIVE_BUFFER_ANDROID};
use crate::egl::types::EGLenum;
use crate::{Error, WindowingApiError};

use euclid::default::Size2D;
use raw_window_handle::RawWindowHandle;
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::ptr;

pub(crate) type NativeBuffer = AHardwareBuffer;
pub(crate) type NativeWindow = ANativeWindow;

/// A surface displayed in an Android native window.
pub struct AndroidWindowSurface(pub(crate) NativeWindowSurface);

/// An offscreen surface backed by an Android `AHardwareBuffer`.
pub struct AndroidBufferSurface(pub(crate) NativeBufferSurface);

impl Debug for AndroidWindowSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "AndroidWindowSurface({:x})", self.0.id().0)
    }
}

impl Debug for AndroidBufferSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "AndroidBufferSurface({:x})", self.0.id().0)
    }
}

/// Returns the native window of a window handle, if it is one of this platform.
pub(crate) fn native_window(window_handle: RawWindowHandle) -> Result<*mut NativeWindow, Error> {
    match window_handle {
        RawWindowHandle::AndroidNdk(handle) => Ok(handle.a_native_window.as_ptr().cast()),
        _ => Err(Error::IncompatibleSurfaceType),
    }
}

pub(crate) fn allocate_native_buffer(size: &Size2D<i32>) -> Result<*mut NativeBuffer, Error> {
    let hardware_buffer_desc = AHardwareBuffer_Desc {
        format: AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
        height: size.height as u32,
        width: size.width as u32,
        layers: 1,
        rfu0: 0,
        rfu1: 0,
        stride: 10,
        usage: AHARDWAREBUFFER_USAGE_CPU_READ_NEVER
            | AHARDWAREBUFFER_USAGE_CPU_WRITE_NEVER
            | AHARDWAREBUFFER_USAGE_GPU_FRAMEBUFFER
            | AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE,
    };
    let mut hardware_buffer = ptr::null_mut();
    let result = unsafe { AHardwareBuffer_allocate(&hardware_buffer_desc, &mut hardware_buffer) };
    if result != 0 {
        return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
    }
    Ok(hardware_buffer)
}

/// # Safety
///
/// `buffer` must be a buffer returned by [`allocate_native_buffer`] that is not released yet.
pub(crate) unsafe fn release_native_buffer(buffer: *mut NativeBuffer) {
    unsafe { AHardwareBuffer_release(buffer) };
}

/// Takes a new reference to a native buffer.
///
/// # Safety
///
/// `buffer` must point to a live native buffer.
pub(crate) unsafe fn acquire_native_buffer(buffer: *mut NativeBuffer) {
    unsafe { AHardwareBuffer_acquire(buffer) };
}

/// # Safety
///
/// `buffer` must point to a live native buffer.
pub(crate) unsafe fn native_buffer_size(buffer: *mut NativeBuffer) -> Size2D<i32> {
    let mut desc: AHardwareBuffer_Desc = unsafe { mem::zeroed() };
    unsafe { AHardwareBuffer_describe(buffer, &mut desc) };
    Size2D::new(desc.width as i32, desc.height as i32)
}

/// # Safety
///
/// `window` must be a valid native window.
pub(crate) unsafe fn native_window_size(window: *mut NativeWindow) -> Size2D<i32> {
    unsafe {
        Size2D::new(
            ANativeWindow_getWidth(window),
            ANativeWindow_getHeight(window),
        )
    }
}

/// Returns the EGL client buffer of a native buffer, and the target to make an EGL image of it
/// with.
///
/// # Safety
///
/// `buffer` must be a buffer returned by [`allocate_native_buffer`] that is not released yet.
#[allow(non_snake_case)]
pub(crate) unsafe fn native_client_buffer(buffer: *mut NativeBuffer) -> (EGLClientBuffer, EGLenum) {
    let eglGetNativeClientBufferANDROID = EGL_EXTENSION_FUNCTIONS
        .GetNativeClientBufferANDROID
        .expect("Where's the `EGL_ANDROID_get_native_client_buffer` extension?");
    let client_buffer = eglGetNativeClientBufferANDROID(buffer.cast_const().cast());
    (client_buffer, EGL_NATIVE_BUFFER_ANDROID)
}
