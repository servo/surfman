// surfman/surfman/src/platform/macos/system/connection.rs
//
//! Represents the connection to the Core Graphics window server.
//! 
//! This is a no-op, because the system APIs automatically manage the global window server
//! connection.

use crate::Error;
use super::device::{Adapter, Device};
use super::surface::{NSView, NativeWidget};

use cocoa::base::id;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::bundle::CFBundleGetInfoDictionary;
use core_foundation::bundle::CFBundleGetMainBundle;
use core_foundation::dictionary::{CFMutableDictionary, CFMutableDictionaryRef};
use core_foundation::string::CFString;
use std::str::FromStr;

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::macos::WindowExt;

/// A no-op connection.
#[derive(Clone)]
pub struct Connection;

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
            let bundle_info_dictionary = CFBundleGetInfoDictionary(main_bundle) as
                CFMutableDictionaryRef;
            assert!(!bundle_info_dictionary.is_null());
            let mut bundle_info_dictionary =
                CFMutableDictionary::wrap_under_get_rule(bundle_info_dictionary);
            let supports_automatic_graphics_switching_key: CFString =
                FromStr::from_str("NSSupportsAutomaticGraphicsSwitching").unwrap();
            let supports_automatic_graphics_switching_value: CFBoolean =
                CFBoolean::true_value();
            bundle_info_dictionary.set(supports_automatic_graphics_switching_key,
                                       supports_automatic_graphics_switching_value);
        }

        Ok(Connection)
    }

    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn create_adapter(&self) -> Result<Adapter, Error> {
        self.create_hardware_adapter()
    }

    /// Returns the "best" hardware adapter on this system.
    /// 
    /// On multi-GPU systems, this will return the discrete GPU.
    #[inline]
    pub fn create_hardware_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter { is_low_power: false })
    }

    /// Returns the most energy-efficient hardware adapter on this system.
    /// 
    /// On multi-GPU systems, this will return the integrated GPU.
    #[inline]
    pub fn create_low_power_adapter(&self) -> Result<Adapter, Error> {
        Ok(Adapter { is_low_power: true })
    }

    /// Returns the "best" software adapter on this system.
    #[inline]
    pub fn create_software_adapter(&self) -> Result<Adapter, Error> {
        self.create_low_power_adapter()
    }

    #[inline]
    pub fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
        Device::new((*adapter).clone())
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(_: &Window) -> Result<Connection, Error> {
        Connection::new()
    }

    #[cfg(feature = "sm-winit")]
    pub fn create_native_widget_from_winit_window(&self, window: &Window)
                                                  -> Result<NativeWidget, Error> {
        let ns_view = window.get_nsview() as id;
        if ns_view.is_null() {
            return Err(Error::IncompatibleNativeWidget);
        }
        unsafe {
            Ok(NativeWidget { view: NSView(msg_send![ns_view, retain]) })
        }
    }
}
