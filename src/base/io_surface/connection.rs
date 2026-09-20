//! Represents the connection to the Core Graphics window server.
//!
//! Connection types are zero-sized on macOS, because the system APIs automatically manage the
//! global window server connection.

use super::adapter::AppleAdapter;
use super::device::{Device, NativeDevice};
use super::surface::NativeWidget;
use crate::adapter::{AdapterPreferences, PowerPreference, RenderingPreference};
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
                supports_automatic_graphics_switching_value as *const _ as *const c_void,
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

    /// Returns an adapter on this system according to the provided preferences.
    #[inline]
    pub fn create_adapter(&self, preferences: AdapterPreferences) -> Result<AppleAdapter, Error> {
        let is_low_power = matches!(preferences.power, PowerPreference::LowPower)
            || matches!(preferences.rendering, RenderingPreference::Software);
        Ok(AppleAdapter { is_low_power })
    }

    /// Opens the hardware device corresponding to the given adapter.
    ///
    /// Device handles are local to a single thread.
    #[inline]
    pub fn create_device(&self, adapter: &AppleAdapter) -> Result<Device, Error> {
        Device::new(adapter.clone())
    }

    /// An alias for `connection.create_device()` with the default adapter.
    #[inline]
    pub unsafe fn create_device_from_native_device(
        &self,
        _: NativeDevice,
    ) -> Result<Device, Error> {
        self.create_device(&self.create_adapter(Default::default())?)
    }

    /// Opens the display connection corresponding to the given `DisplayHandle`.
    pub fn from_display_handle(_: raw_window_handle::DisplayHandle) -> Result<Connection, Error> {
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

    /// Create a native widget type from the given `WindowHandle`.
    #[inline]
    pub fn create_native_widget_from_window_handle(
        &self,
        handle: raw_window_handle::WindowHandle,
        _size: Size2D<i32>,
    ) -> Result<NativeWidget, Error> {
        use objc2::{MainThreadMarker, Message};
        use raw_window_handle::RawWindowHandle::AppKit;

        match handle.as_raw() {
            AppKit(handle) => {
                assert!(
                    MainThreadMarker::new().is_some(),
                    "NSView is only usable on the main thread"
                );
                // SAFETY: The pointer is valid for as long as the handle is,
                // and we just checked that we're on the main thread.
                let ns_view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
                let ns_window = ns_view
                    .window()
                    .expect("view must be installed in a window");
                Ok(NativeWidget {
                    // Extend the lifetime of the view.
                    view: ns_view.retain(),
                    opaque: ns_window.isOpaque(),
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
