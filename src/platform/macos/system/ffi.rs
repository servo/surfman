// surfman/surfman/src/platform/macos/system/ffi.rs
//
//! FFI definitions for the macOS backend.

#![allow(non_upper_case_globals)]

pub(crate) const kIODefaultCache: i32 = 0;
pub(crate) const kIOWriteCombineCache: i32 = 4;
pub(crate) const kIOMapCacheShift: i32 = 8;
pub(crate) const kIOMapDefaultCache: i32 = kIODefaultCache << kIOMapCacheShift;
pub(crate) const kIOMapWriteCombineCache: i32 = kIOWriteCombineCache << kIOMapCacheShift;
