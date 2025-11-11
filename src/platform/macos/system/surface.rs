// surfman/surfman/src/platform/macos/system/surface.rs
//
//! Surface management for macOS.

use super::device::Device;
use super::ffi::{kIOMapDefaultCache, kIOMapWriteCombineCache};
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceType, SystemSurfaceInfo};

use euclid::default::Size2D;
use libc::KERN_SUCCESS;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_foundation::{
    kCFAllocatorDefault, kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    CFDictionary, CFIndex, CFNumber, CFRetained, CFString, CGPoint, CGRect, CGSize,
};
// CVDisplayLink is deprecated, but the replaced APIs are only available
// on newer OS versions.
#[allow(deprecated)]
use objc2_core_video::{
    kCVPixelFormatType_32BGRA, kCVReturnSuccess, CVDisplayLink, CVDisplayLinkCreateWithCGDisplay,
    CVDisplayLinkSetOutputCallback, CVDisplayLinkStart, CVDisplayLinkStop, CVOptionFlags, CVReturn,
    CVTimeStamp,
};
use objc2_foundation::{ns_string, NSNumber, NSPoint, NSRect, NSSize};
use objc2_io_surface::{
    kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, kIOSurfaceCacheMode, kIOSurfaceHeight,
    kIOSurfacePixelFormat, kIOSurfaceWidth, IOSurfaceLockOptions, IOSurfaceRef,
};
use objc2_quartz_core::{CALayer, CATransaction, CATransform3D};
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::os::raw::c_void;
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

const BYTES_PER_PIXEL: i32 = 4;

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
///
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
pub struct Surface {
    pub(crate) io_surface: CFRetained<IOSurfaceRef>,
    pub(crate) size: Size2D<i32>,
    access: SurfaceAccess,
    pub(crate) destroyed: bool,
    pub(crate) view_info: Option<ViewInfo>,
}

/// A wrapper around an `IOSurfaceRef`.
#[derive(Clone)]
pub struct NativeSurface(pub CFRetained<IOSurfaceRef>);

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

pub(crate) struct ViewInfo {
    view: Retained<NSView>,
    layer: Retained<CALayer>,
    superlayer: Retained<CALayer>,
    front_surface: CFRetained<IOSurfaceRef>,
    logical_size: NSSize,
    display_link: CFRetained<CVDisplayLink>,
    next_vblank: Arc<VblankCond>,
    opaque: bool,
}

struct VblankCond {
    mutex: Mutex<()>,
    cond: Condvar,
}

/// A native widget on macOS (`NSView`).
#[derive(Clone)]
pub struct NativeWidget {
    /// The `NSView` object.
    pub view: Retained<NSView>,
    /// A bool value that indicates whether widget's NSWindow is opaque.
    pub opaque: bool,
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    surface: &'a mut Surface,
    stride: usize,
    ptr: NonNull<u8>,
    len: usize,
}

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    pub fn create_surface(
        &self,
        access: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        unsafe {
            let size = match surface_type {
                SurfaceType::Generic { size } => size,
                SurfaceType::Widget { ref native_widget } => {
                    let window = native_widget
                        .view
                        .window()
                        .expect("view must be installed in a window");
                    let bounds = window.convertRectToBacking(native_widget.view.bounds());

                    // The surface will not appear if its width is not a multiple of 4 (i.e. stride
                    // is a multiple of 16 bytes). Enforce this.
                    let mut width = bounds.size.width as i32;
                    let height = bounds.size.height as i32;
                    if width % 4 != 0 {
                        width += 4 - width % 4;
                    }

                    Size2D::new(width, height)
                }
            };

            let io_surface = self.create_io_surface(&size, access);

            let view_info = match surface_type {
                SurfaceType::Generic { .. } => None,
                SurfaceType::Widget {
                    ref native_widget, ..
                } => Some(self.create_view_info(&size, access, native_widget)),
            };

            Ok(Surface {
                io_surface,
                size,
                access,
                destroyed: false,
                view_info,
            })
        }
    }

    pub(crate) fn set_surface_flipped(&self, surface: &mut Surface, flipped: bool) {
        if let Some(ref mut view_info) = surface.view_info {
            let (scale_y, translate_y) = if flipped {
                (-1.0, -view_info.logical_size.height)
            } else {
                (1.0, 0.0)
            };

            let sublayer_transform =
                CATransform3D::new_scale(1.0, scale_y, 1.0).translate(0.0, translate_y, 0.0);
            view_info
                .superlayer
                .setSublayerTransform(sublayer_transform);
        }
    }

    #[allow(deprecated)]
    unsafe fn create_view_info(
        &self,
        size: &Size2D<i32>,
        surface_access: SurfaceAccess,
        native_widget: &NativeWidget,
    ) -> ViewInfo {
        let front_surface = self.create_io_surface(size, surface_access);

        let window = native_widget
            .view
            .window()
            .expect("view must be installed in a window");
        let device_description = window.screen().unwrap().deviceDescription();
        let display_id = device_description
            .objectForKey(ns_string!("NSScreenNumber"))
            .unwrap()
            .downcast::<NSNumber>()
            .unwrap()
            .as_u32();
        let mut display_link = ptr::null_mut();
        assert_eq!(
            CVDisplayLinkCreateWithCGDisplay(display_id, NonNull::from(&mut display_link)),
            kCVReturnSuccess,
        );
        let display_link = CFRetained::from_raw(NonNull::new(display_link).unwrap());
        let next_vblank = Arc::new(VblankCond {
            mutex: Mutex::new(()),
            cond: Condvar::new(),
        });
        CVDisplayLinkSetOutputCallback(
            &display_link,
            Some(display_link_output_callback),
            mem::transmute(next_vblank.clone()),
        );
        CVDisplayLinkStart(&display_link);

        CATransaction::begin();
        CATransaction::setDisableActions(true);

        let superlayer = CALayer::new();
        native_widget.view.setLayer(Some(&superlayer));
        native_widget.view.setWantsLayer(true);

        // Compute logical size.
        let window = native_widget
            .view
            .window()
            .expect("view must be installed in a window");
        let logical_rect = window.convertRectFromBacking(NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize {
                width: size.width as f64,
                height: size.height as f64,
            },
        });
        let logical_size = logical_rect.size;

        let opaque = native_widget.opaque;
        let layer = CALayer::new();
        let layer_size = CGSize::new(logical_size.width as f64, logical_size.height as f64);
        layer.setFrame(CGRect::new(CGPoint::ZERO, layer_size));
        layer.setContents(Some(front_surface.as_ref()));
        layer.setOpaque(opaque);
        // TODO: The `contentsOpaque` property does not exist?
        let _: () = unsafe { msg_send![&layer, setContentsOpaque: opaque] };
        superlayer.addSublayer(&layer);

        let view = native_widget.view.clone();
        CATransaction::commit();

        ViewInfo {
            view,
            layer,
            superlayer,
            front_surface,
            logical_size,
            display_link,
            next_vblank,
            opaque,
        }
    }

    /// Destroys a surface.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(&self, surface: &mut Surface) -> Result<(), Error> {
        surface.destroyed = true;
        Ok(())
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    pub fn present_surface(&self, surface: &mut Surface) -> Result<(), Error> {
        surface.present()
    }

    /// Resizes a widget surface
    pub fn resize_surface(
        &self,
        surface: &mut Surface,
        mut size: Size2D<i32>,
    ) -> Result<(), Error> {
        // The surface will not appear if its width is not a multiple of 4 (i.e. stride is a
        // multiple of 16 bytes). Enforce this.
        let width = size.width as i32;
        if width % 4 != 0 {
            size.width = width + 4 - width % 4;
        }

        let view_info = match surface.view_info {
            None => return Err(Error::NoWidgetAttached),
            Some(ref mut view_info) => view_info,
        };

        CATransaction::begin();
        CATransaction::setDisableActions(true);

        // Compute logical size.
        let window = view_info
            .view
            .window()
            .expect("view must be installed in a window");
        let logical_rect = unsafe {
            window.convertRectFromBacking(NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size: NSSize {
                    width: size.width as f64,
                    height: size.height as f64,
                },
            })
        };
        let logical_size = logical_rect.size;
        let layer_size = CGSize::new(logical_size.width as f64, logical_size.height as f64);

        // Flip contents right-side-up.
        let sublayer_transform =
            CATransform3D::new_scale(1.0, -1.0, 1.0).translate(0.0, -logical_size.height, 0.0);
        view_info
            .superlayer
            .setSublayerTransform(sublayer_transform);

        view_info.front_surface = self.create_io_surface(&size, surface.access);
        view_info
            .layer
            .setFrame(CGRect::new(CGPoint::ZERO, layer_size));
        unsafe {
            view_info
                .layer
                .setContents(Some(view_info.front_surface.as_ref()))
        };
        view_info.layer.setOpaque(view_info.opaque);
        // TODO: The `contentsOpaque` property does not exist?
        let _: () = unsafe { msg_send![&view_info.layer, setContentsOpaque: view_info.opaque] };
        surface.io_surface = self.create_io_surface(&size, surface.access);
        surface.size = size;

        CATransaction::commit();
        Ok(())
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(
        &self,
        surface: &'s mut Surface,
    ) -> Result<SurfaceDataGuard<'s>, Error> {
        surface.lock_data()
    }

    fn create_io_surface(
        &self,
        size: &Size2D<i32>,
        access: SurfaceAccess,
    ) -> CFRetained<IOSurfaceRef> {
        let cache_mode = match access {
            SurfaceAccess::GPUCPUWriteCombined => kIOMapWriteCombineCache,
            SurfaceAccess::GPUOnly | SurfaceAccess::GPUCPU => kIOMapDefaultCache,
        };

        unsafe {
            let bytes_per_row = IOSurfaceRef::align_property(
                kIOSurfaceBytesPerRow,
                (size.width * BYTES_PER_PIXEL) as usize,
            ) as i32;
            let keys = [
                kIOSurfaceWidth,
                kIOSurfaceHeight,
                kIOSurfaceBytesPerElement,
                kIOSurfaceBytesPerRow,
                kIOSurfacePixelFormat,
                kIOSurfaceCacheMode,
            ];
            let values = [
                &*CFNumber::new_i32(size.width),
                &*CFNumber::new_i32(size.height),
                &*CFNumber::new_i32(BYTES_PER_PIXEL),
                &*CFNumber::new_i32(bytes_per_row),
                &*CFNumber::new_i32(kCVPixelFormatType_32BGRA as i32),
                &*CFNumber::new_i32(cache_mode),
            ];
            assert_eq!(keys.len(), values.len());
            let len = keys.len() as CFIndex;

            // CFNumber and CFString are CF types, and we specify
            // `kCFTypeDictionaryKeyCallBacks` and
            // `kCFTypeDictionaryValueCallBacks`.
            let keys: *const &CFString = keys.as_ptr();
            let keys: *mut *const c_void = keys as _;
            let values: *const &CFNumber = values.as_ptr();
            let values: *mut *const c_void = values as _;

            let properties = CFDictionary::new(
                kCFAllocatorDefault,
                keys,
                values,
                len,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
            .unwrap();

            IOSurfaceRef::new(&properties).unwrap()
        }
    }

    /// Returns various information about the surface.
    #[inline]
    pub fn surface_info(&self, surface: &Surface) -> SystemSurfaceInfo {
        SystemSurfaceInfo {
            size: surface.size,
            id: surface.id(),
        }
    }

    /// Returns the native `IOSurfaceRef` corresponding to this surface.
    #[inline]
    pub fn native_surface(&self, surface: &Surface) -> NativeSurface {
        let io_surface = surface.io_surface.clone();
        NativeSurface(io_surface)
    }
}

impl Surface {
    #[inline]
    fn id(&self) -> SurfaceID {
        SurfaceID(&*self.io_surface as *const IOSurfaceRef as usize)
    }

    fn present(&mut self) -> Result<(), Error> {
        unsafe {
            CATransaction::begin();
            CATransaction::setDisableActions(true);

            let view_info = match self.view_info {
                None => return Err(Error::NoWidgetAttached),
                Some(ref mut view_info) => view_info,
            };
            mem::swap(&mut view_info.front_surface, &mut self.io_surface);
            view_info
                .layer
                .setContents(Some(view_info.front_surface.as_ref()));

            CATransaction::commit();

            // Wait for the next swap interval.
            let next_vblank_mutex_guard = view_info.next_vblank.mutex.lock().unwrap();
            drop(
                view_info
                    .next_vblank
                    .cond
                    .wait(next_vblank_mutex_guard)
                    .unwrap(),
            );

            Ok(())
        }
    }

    pub(crate) fn lock_data(&mut self) -> Result<SurfaceDataGuard, Error> {
        if !self.access.cpu_access_allowed() {
            return Err(Error::SurfaceDataInaccessible);
        }

        unsafe {
            let mut seed = 0;
            let result = self
                .io_surface
                .lock(IOSurfaceLockOptions::empty(), &mut seed);
            if result != KERN_SUCCESS {
                return Err(Error::SurfaceLockFailed);
            }

            let ptr = self.io_surface.base_address().cast::<u8>();
            let len = self.io_surface.alloc_size();
            let stride = self.io_surface.bytes_per_row();

            Ok(SurfaceDataGuard {
                surface: &mut *self,
                stride,
                ptr,
                len,
            })
        }
    }
}

impl<'a> SurfaceDataGuard<'a> {
    /// Returns the number of bytes per row of the surface.
    #[inline]
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Returns a mutable slice of the pixel data in this surface, in BGRA format.
    #[inline]
    pub fn data(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl<'a> Drop for SurfaceDataGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let mut seed = 0;
            self.surface
                .io_surface
                .unlock(IOSurfaceLockOptions::empty(), &mut seed);
        }
    }
}

impl Drop for ViewInfo {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // FIXME(pcwalton): When this returns, are there absolutely guaranteed to be no more
            // callbacks? `CVDisplayLinkStop()` documentation doesn't say…
            //
            // If not, then this is a possible use-after-free!
            #[allow(deprecated)]
            CVDisplayLinkStop(&self.display_link);

            // Drop the reference that the callback was holding onto.
            let _ = mem::transmute_copy::<Arc<VblankCond>, Arc<VblankCond>>(&self.next_vblank);

            self.layer.removeFromSuperlayer();
        }
    }
}

unsafe extern "C-unwind" fn display_link_output_callback(
    _display_link: NonNull<CVDisplayLink>,
    _in_now: NonNull<CVTimeStamp>,
    _in_output_time: NonNull<CVTimeStamp>,
    _flags_n: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    display_link_context: *mut c_void,
) -> CVReturn {
    let next_vblank: Arc<VblankCond> = mem::transmute(display_link_context);
    {
        let _guard = next_vblank.mutex.lock().unwrap();
        next_vblank.cond.notify_all();
    }

    mem::forget(next_vblank);
    kCVReturnSuccess
}
