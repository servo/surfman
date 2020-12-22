// surfman/surfman/src/platform/macos/system/ffi.rs
//
//! FFI definitions for the macOS backend.

#![allow(non_upper_case_globals)]

use io_surface::IOSurfaceRef;
use mach::kern_return::kern_return_t;
use std::os::raw::c_void;

pub(crate) const kCVPixelFormatType_32BGRA: i32 = 0x42475241; // 'BGRA'

pub(crate) const kCVReturnSuccess: i32 = 0;

pub(crate) const kIODefaultCache: i32 = 0;
pub(crate) const kIOWriteCombineCache: i32 = 4;
pub(crate) const kIOMapCacheShift: i32 = 8;
pub(crate) const kIOMapDefaultCache: i32 = kIODefaultCache << kIOMapCacheShift;
pub(crate) const kIOMapWriteCombineCache: i32 = kIOWriteCombineCache << kIOMapCacheShift;

pub(crate) type IOSurfaceLockOptions = u32;

#[link(name = "IOSurface", kind = "framework")]
extern "C" {
    pub(crate) fn IOSurfaceGetAllocSize(buffer: IOSurfaceRef) -> usize;
    pub(crate) fn IOSurfaceGetBaseAddress(buffer: IOSurfaceRef) -> *mut c_void;
    pub(crate) fn IOSurfaceGetBytesPerRow(buffer: IOSurfaceRef) -> usize;
    pub(crate) fn IOSurfaceLock(
        buffer: IOSurfaceRef,
        options: IOSurfaceLockOptions,
        seed: *mut u32,
    ) -> kern_return_t;
    pub(crate) fn IOSurfaceUnlock(
        buffer: IOSurfaceRef,
        options: IOSurfaceLockOptions,
        seed: *mut u32,
    ) -> kern_return_t;
}
