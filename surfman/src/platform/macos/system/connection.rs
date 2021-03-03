// surfman/surfman/src/platform/macos/system/connection.rs
//
//! Represents the connection to the Core Graphics window server.
//!
//! Connection types are zero-sized on macOS, because the system APIs automatically manage the
//! global window server connection.

use super::device::{Adapter, Device, NativeDevice};
use super::surface::{NSView, NativeWidget};
use crate::Error;

use cocoa::base::id;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::bundle::CFBundleGetInfoDictionary;
use core_foundation::bundle::CFBundleGetMainBundle;
use core_foundation::dictionary::{CFMutableDictionary, CFMutableDictionaryRef};
use core_foundation::string::CFString;

use euclid::default::Size2D;

use std::os::raw::c_void;
use std::str::FromStr;

#[cfg(feature = "sm-winit")]
use winit::platform::macos::WindowExtMacOS;
#[cfg(feature = "sm-winit")]
use winit::window::Window;

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
            // can opt into the integrated GPU if available. This is a total hack, as there's no
            // guarantee `Info.plist` dictionaries are mutable.
            let main_bundle = CFBundleGetMainBundle();
            assert!(!main_bundle.is_null());
            let bundle_info_dictionary =
                CFBundleGetInfoDictionary(main_bundle) as CFMutableDictionaryRef;
            assert!(!bundle_info_dictionary.is_null());
            let mut bundle_info_dictionary =
                CFMutableDictionary::wrap_under_get_rule(bundle_info_dictionary);
            let supports_automatic_graphics_switching_key: CFString =
                FromStr::from_str("NSSupportsAutomaticGraphicsSwitching").unwrap();
            let supports_automatic_graphics_switching_value: CFBoolean = CFBoolean::true_value();
            bundle_info_dictionary.set(
                supports_automatic_graphics_switching_key,
                supports_automatic_graphics_switching_value,
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

    /// Opens the display connection corresponding to the given `winit` window.
    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Connection::new()
    }

    /// Creates a native widget type from the given `winit` window.
    ///
    /// This type can be later used to create surfaces that render to the window.
    #[cfg(feature = "sm-winit")]
    pub fn create_native_widget_from_winit_window(
        &self,
        window: &Window,
    ) -> Result<NativeWidget, Error> {
        let ns_view = window.ns_view() as id;
        if ns_view.is_null() {
            return Err(Error::IncompatibleNativeWidget);
        }
        unsafe {
            Ok(NativeWidget {
                view: NSView(msg_send![ns_view, retain]),
            })
        }
    }

    /// Create a native widget from a raw pointer
    pub unsafe fn create_native_widget_from_ptr(
        &self,
        raw: *mut c_void,
        _size: Size2D<i32>,
    ) -> NativeWidget {
        NativeWidget {
            view: NSView(raw as id),
        }
    }

    /// Create a native widget type from the given `raw_window_handle::RawWindowHandle`.
    #[cfg(feature = "sm-raw-window-handle")]
    #[inline]
    pub fn create_native_widget_from_rwh(
        &self,
        raw_handle: raw_window_handle::RawWindowHandle,
    ) -> Result<NativeWidget, Error> {
        use raw_window_handle::RawWindowHandle::MacOS;

        match raw_handle {
            MacOS(handle) => Ok(NativeWidget {
                view: NSView(unsafe { msg_send![handle.ns_view as id, retain] }),
            }),
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
