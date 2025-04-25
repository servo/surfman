// surfman/surfman/src/platform/macos/system/connection.rs
//
//! Represents the connection to the Core Graphics window server.
//!
//! Connection types are zero-sized on macOS, because the system APIs automatically manage the
//! global window server connection.

use super::device::{Adapter, Device, NativeDevice};
use super::surface::NativeWidget;
use crate::Error;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_foundation::{CFBoolean, CFBundle, CFMutableDictionary, CFRetained, CFString};

use euclid::default::Size2D;

use std::os::raw::c_void;

/// A no-op connection.
///
/// Connections to the CGS window server are implicit on macOS, so this is a zero-sized type.
#[derive(Clone)]
pub struct Connection;

/// An empty placeholder for native connections.
///
/// Connections to the CGS window server are implicit on macOS, so this is a zero-sized type.
#[derive(Clone)]
pub struct NativeConnection;

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        unsafe {
            // Adjust the `NSSupportsAutomaticGraphicsSwitching` key in our `Info.plist` so that we
            // can opt into the integrated GPU if available.
            let main_bundle = CFBundle::main_bundle().unwrap();
            let bundle_info_dictionary = main_bundle.info_dictionary().unwrap();

            // This is a total hack, as there's no guarantee `Info.plist` dictionaries are mutable.
            let bundle_info_dictionary =
                CFRetained::cast_unchecked::<CFMutableDictionary>(bundle_info_dictionary);

            let supports_automatic_graphics_switching_key =
                CFString::from_str("NSSupportsAutomaticGraphicsSwitching");
            let supports_automatic_graphics_switching_value = CFBoolean::new(true);
            CFMutableDictionary::set_value(
                Some(&bundle_info_dictionary),
                &*supports_automatic_graphics_switching_key as *const _ as *const c_void,
                &*supports_automatic_graphics_switching_value as *const _ as *const c_void,
            );
        }

        Ok(Connection)
    }

    /// An alias for `Connection::new()`, present for consistency with other backends.
    #[inline]
    pub unsafe fn from_native_connection(_: NativeConnection) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Returns the underlying native connection.
    #[inline]
    pub fn native_connection(&self) -> NativeConnection {
        NativeConnection
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    ///
    /// This is an alias for `Connection::create_hardware_adapter()`.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" adapter on this system, preferring high-performance hardware adapters.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter {
            is_low_power: false,
        })
    }

    /// Returns the "best" adapter on this system, preferring low-power hardware adapters.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter { is_low_power: true })
    }

    /// Returns the "best" adapter on this system, preferring software adapters.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        self.create_low_power_adapter()
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new((*adapter).clone())
    }

    /// An alias for `connection.create_device()` with the default adapter.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        _: NativeDevice,
    ) -> Result<Device, Error> {
        self.create_device(&self.create_adapter()?)
    }

    /// Opens the display connection corresponding to the given `RawDisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    pub fn from_raw_display_handle(_: rwh_05::RawDisplayHandle) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    pub fn from_display_handle(_: rwh_06::DisplayHandle) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        _size: Size2D<i32>,
    ) -> NativeWidget {
        let view_ptr: *mut NSView = raw.cast();
        NativeWidget {
            // SAFETY: Validity of the NSView is upheld by caller.
            // TODO(madsmtm): We should probably `retain` here, rather than
            // take ownership of the pointer.
            view: unsafe { Retained::from_raw(view_ptr).unwrap() },
            opaque: true,
        }
    }

    /// Create a native widget type from the given `RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-05")]
    #[inline]
    pub fn create_native_widget_from_raw_window_handle(
        &self,
        raw_handle: rwh_05::RawWindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use objc2::{MainThreadMarker, Message};
        use objc2_app_kit::NSWindow;
        use rwh_05::RawWindowHandle::AppKit;

        match raw_handle {
            AppKit(handle) => {
                assert!(
                    MainThreadMarker::new().is_some(),
                    "NSView is only usable on the main thread"
                );
                // SAFETY: The pointer is valid for as long as the handle is,
                // and we just checked that we're on the main thread.
                let ns_view = unsafe { handle.ns_view.cast::<NSView>().as_ref().unwrap() };
                let ns_window = unsafe { handle.ns_window.cast::<NSWindow>().as_ref().unwrap() };

                Ok(NativeWidget {
                    view: ns_view.retain(),
                    opaque: unsafe { ns_window.isOpaque() },
                })
            }
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }

    /// Create a native widget type from the given `WindowHandle`.
    #[cfg(feature = "sm-raw-window-handle-06")]
    #[inline]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: rwh_06::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use objc2::{MainThreadMarker, Message};
        use rwh_06::RawWindowHandle::AppKit;

        match handle.as_raw() {
            AppKit(handle) => {
                assert!(
                    MainThreadMarker::new().is_some(),
                    "NSView is only usable on the main thread"
                );
                // SAFETY: The pointer is valid for as long as the handle is,
                // and we just checked that we're on the main thread.
                let ns_view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
                let ns_window = ns_view.window().expect("view must be in window");
                Ok(NativeWidget {
                    // Extend the lifetime of the view.
                    view: ns_view.retain(),
                    opaque: unsafe { ns_window.isOpaque() },
                })
            }
            _ => Err(Error::IncompatibleNativeWidget),
        }
    }
}

impl NativeConnection {
    /// Returns the current native connection.
    ///
    /// This is a no-op on macOS, because Core Graphics window server connections are implicit in
    /// the platform APIs.
    #[inline]
    pub fn current() -> Result<NativeConnection, Error> {
        Ok(NativeConnection)
    }
}
