//! Native handles to the memory of buffer surfaces, for sharing it with other devices, graphics
//! APIs and processes.

#[cfg(any(android_platform, ohos_platform))]
pub use crate::hardware_buffer::surface::NativeHardwareBuffer;
#[cfg(macos_platform)]
use objc2_core_foundation::CFRetained;
#[cfg(macos_platform)]
use objc2_io_surface::IOSurfaceRef;
#[cfg(windows_platform)]
use winapi::um::winnt::HANDLE;

/// A native handle to the memory of a buffer surface.
///
/// A handle comes from [`crate::Device::buffer_handle`] and can be imported into another device
/// with [`crate::Device::import_buffer_handle`], or into another graphics API that understands
/// the native handle. The handle refers to the same memory as the surface: whatever is rendered
/// to one is visible through the other, so access from different devices or APIs must be ordered
/// by the caller.
#[derive(Debug)]
pub enum BufferHandle {
    /// An `IOSurface`, on macOS.
    #[cfg(macos_platform)]
    IoSurface(CFRetained<IOSurfaceRef>),
    /// The DXGI share handle of a Direct3D 11 texture, as returned by
    /// `IDXGIResource::GetSharedHandle`. It is not an NT handle and must not be closed.
    #[cfg(windows_platform)]
    D3DTexture(HANDLE),
    /// An Android `AHardwareBuffer` or an OpenHarmony `OH_NativeBuffer`.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(NativeHardwareBuffer),
}
