// surfman/surfman/src/platform/macos/system/surface.rs
//
//! Surface management for macOS.

use super::device::Device;
use super::ffi::{kCVPixelFormatType_32BGRA, kIOMapDefaultCache, IOSurfaceLock, IOSurfaceUnlock};
use super::ffi::{kCVReturnSuccess, kIOMapWriteCombineCache};
use super::ffi::{IOSurfaceGetAllocSize, IOSurfaceGetBaseAddress, IOSurfaceGetBytesPerRow};
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceType, SystemSurfaceInfo};

use cocoa::appkit::{NSScreen, NSView as NSViewMethods, NSWindow};
use cocoa::base::{id, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use cocoa::quartzcore::{transaction, CALayer, CATransform3D};
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGRect, CGSize, CG_ZERO_POINT};
use euclid::default::Size2D;
use io_surface::{self, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, IOSurface, IOSurfaceRef};
use io_surface::{kIOSurfaceCacheMode, kIOSurfaceHeight, kIOSurfacePixelFormat, kIOSurfaceWidth};
use mach::kern_return::KERN_SUCCESS;
use servo_display_link::macos::cvdisplaylink::{CVDisplayLink, CVTimeStamp, DisplayLink};
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::os::raw::c_void;
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
    pub(crate) io_surface: IOSurface,
    pub(crate) size: Size2D<i32>,
    access: SurfaceAccess,
    pub(crate) destroyed: bool,
    pub(crate) view_info: Option<ViewInfo>,
}

/// A wrapper around an `IOSurface`.
#[derive(Clone)]
pub struct NativeSurface(pub IOSurfaceRef);

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
    view: NSView,
    layer: CALayer,
    superlayer: CALayer,
    front_surface: IOSurface,
    logical_size: NSSize,
    display_link: DisplayLink,
    next_vblank: Arc<VblankCond>,
}

struct VblankCond {
    mutex: Mutex<()>,
    cond: Condvar,
}

/// Wraps an `NSView` object.
#[derive(Clone)]
pub struct NSView(pub(crate) id);

/// A native widget on macOS (`NSView`).
#[derive(Clone)]
pub struct NativeWidget {
    /// The `NSView` object.
    pub view: NSView,
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    surface: &'a mut Surface,
    stride: usize,
    ptr: *mut u8,
    len: usize,
}

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    pub fn create_surface(
        &mut self,
        access: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        unsafe {
            let size = match surface_type {
                SurfaceType::Generic { size } => size,
                SurfaceType::Widget { ref native_widget } => {
                    let window: id = msg_send![native_widget.view.0, window];
                    let bounds = window.convertRectToBacking(native_widget.view.0.bounds());

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
                CATransform3D::from_scale(1.0, scale_y, 1.0).translate(0.0, translate_y, 0.0);
            view_info
                .superlayer
                .set_sublayer_transform(sublayer_transform);
        }
    }

    unsafe fn create_view_info(
        &mut self,
        size: &Size2D<i32>,
        surface_access: SurfaceAccess,
        native_widget: &NativeWidget,
    ) -> ViewInfo {
        let front_surface = self.create_io_surface(&size, surface_access);

        let window: id = msg_send![native_widget.view.0, window];
        let device_description: CFDictionary<CFString, CFNumber> =
            CFDictionary::wrap_under_get_rule(window.screen().deviceDescription() as *const _);
        let description_key: CFString = CFString::from("NSScreenNumber");
        let display_id = device_description.get(description_key).to_i64().unwrap() as u32;
        let mut display_link = DisplayLink::on_display(display_id).unwrap();
        let next_vblank = Arc::new(VblankCond {
            mutex: Mutex::new(()),
            cond: Condvar::new(),
        });
        display_link.set_output_callback(
            display_link_output_callback,
            mem::transmute(next_vblank.clone()),
        );
        display_link.start();

        transaction::begin();
        transaction::set_disable_actions(true);

        let superlayer = CALayer::new();
        native_widget.view.0.setLayer(superlayer.id());
        native_widget.view.0.setWantsLayer(YES);

        // Compute logical size.
        let window: id = msg_send![native_widget.view.0, window];
        let logical_rect: NSRect = msg_send![window, convertRectFromBacking:NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize { width: size.width as f64, height: size.height as f64 },
        }];
        let logical_size = logical_rect.size;

        let layer = CALayer::new();
        let layer_size = CGSize::new(logical_size.width as f64, logical_size.height as f64);
        layer.set_frame(&CGRect::new(&CG_ZERO_POINT, &layer_size));
        layer.set_contents(front_surface.obj as id);
        layer.set_opaque(true);
        layer.set_contents_opaque(true);
        superlayer.add_sublayer(&layer);

        let view = native_widget.view.clone();
        transaction::commit();

        ViewInfo {
            view,
            layer,
            superlayer,
            front_surface,
            logical_size,
            display_link,
            next_vblank,
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

        transaction::begin();
        transaction::set_disable_actions(true);

        unsafe {
            // Compute logical size.
            let window: id = msg_send![view_info.view.0, window];
            let logical_rect: NSRect = msg_send![window, convertRectFromBacking:NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size: NSSize { width: size.width as f64, height: size.height as f64 },
            }];
            let logical_size = logical_rect.size;
            let layer_size = CGSize::new(logical_size.width as f64, logical_size.height as f64);

            // Flip contents right-side-up.
            let sublayer_transform =
                CATransform3D::from_scale(1.0, -1.0, 1.0).translate(0.0, -logical_size.height, 0.0);
            view_info
                .superlayer
                .set_sublayer_transform(sublayer_transform);

            view_info.front_surface = self.create_io_surface(&size, surface.access);
            view_info
                .layer
                .set_frame(&CGRect::new(&CG_ZERO_POINT, &layer_size));
            view_info
                .layer
                .set_contents(view_info.front_surface.obj as id);
            view_info.layer.set_opaque(true);
            view_info.layer.set_contents_opaque(true);
            surface.io_surface = self.create_io_surface(&size, surface.access);
            surface.size = size;
        }

        transaction::commit();
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

    fn create_io_surface(&self, size: &Size2D<i32>, access: SurfaceAccess) -> IOSurface {
        let cache_mode = match access {
            SurfaceAccess::GPUCPUWriteCombined => kIOMapWriteCombineCache,
            SurfaceAccess::GPUOnly | SurfaceAccess::GPUCPU => kIOMapDefaultCache,
        };

        unsafe {
            let properties = CFDictionary::from_CFType_pairs(&[
                (
                    CFString::wrap_under_get_rule(kIOSurfaceWidth),
                    CFNumber::from(size.width).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kIOSurfaceHeight),
                    CFNumber::from(size.height).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                    CFNumber::from(BYTES_PER_PIXEL).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                    CFNumber::from(size.width * BYTES_PER_PIXEL).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kIOSurfacePixelFormat),
                    CFNumber::from(kCVPixelFormatType_32BGRA).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kIOSurfaceCacheMode),
                    CFNumber::from(cache_mode).as_CFType(),
                ),
            ]);

            io_surface::new(&properties)
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

    /// Returns the native `IOSurface` corresponding to this surface.
    ///
    /// The reference count is increased on the `IOSurface` before returning.
    #[inline]
    pub fn native_surface(&self, surface: &Surface) -> NativeSurface {
        let io_surface = surface.io_surface.clone();
        let io_surface_ref = io_surface.as_concrete_TypeRef();
        mem::forget(io_surface);
        NativeSurface(io_surface_ref)
    }
}

impl Surface {
    #[inline]
    fn id(&self) -> SurfaceID {
        SurfaceID(self.io_surface.as_concrete_TypeRef() as usize)
    }

    fn present(&mut self) -> Result<(), Error> {
        unsafe {
            transaction::begin();
            transaction::set_disable_actions(true);

            let view_info = match self.view_info {
                None => return Err(Error::NoWidgetAttached),
                Some(ref mut view_info) => view_info,
            };
            mem::swap(&mut view_info.front_surface, &mut self.io_surface);
            view_info
                .layer
                .set_contents(view_info.front_surface.obj as id);

            transaction::commit();

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
            let result = IOSurfaceLock(self.io_surface.as_concrete_TypeRef(), 0, &mut seed);
            if result != KERN_SUCCESS {
                return Err(Error::SurfaceLockFailed);
            }

            let ptr = IOSurfaceGetBaseAddress(self.io_surface.as_concrete_TypeRef()) as *mut u8;
            let len = IOSurfaceGetAllocSize(self.io_surface.as_concrete_TypeRef());
            let stride = IOSurfaceGetBytesPerRow(self.io_surface.as_concrete_TypeRef());

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
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<'a> Drop for SurfaceDataGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let mut seed = 0;
            IOSurfaceUnlock(self.surface.io_surface.as_concrete_TypeRef(), 0, &mut seed);
        }
    }
}

impl Drop for NativeWidget {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.view.0, release];
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
            self.display_link.stop();

            // Drop the reference that the callback was holding onto.
            mem::transmute_copy::<Arc<VblankCond>, Arc<VblankCond>>(&self.next_vblank);
        }
    }
}

unsafe extern "C" fn display_link_output_callback(
    _: *mut CVDisplayLink,
    _: *const CVTimeStamp,
    _: *const CVTimeStamp,
    _: i64,
    _: *mut i64,
    user_data: *mut c_void,
) -> i32 {
    let next_vblank: Arc<VblankCond> = mem::transmute(user_data);
    {
        let _guard = next_vblank.mutex.lock().unwrap();
        next_vblank.cond.notify_all();
    }

    mem::forget(next_vblank);
    kCVReturnSuccess
}
