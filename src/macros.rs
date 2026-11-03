//! Macros for use in the top-level crate.

/// When using `surfman`, you should place this macro at the top of your crate, like so:
///
/// ```ignore
/// use surfman::macros::declare_surfman;
///
/// declare_surfman!();
///
/// fn main() { ... }
/// ```
///
/// On Windows, this macro exports various linker flags that the GPU drivers look at to determine
/// whether to use the integrated or discrete GPU. If you don't use this macro, `surfman` should
/// still work, but you may get the wrong GPU.
#[macro_export]
macro_rules! declare_surfman {
    () => {
        #[cfg(target_os = "windows")]
        #[link_section = ".drectve"]
        #[no_mangle]
        pub static _SURFMAN_LINK_ARGS: [u8; 74] =
            *b" /export:NvOptimusEnablement /export:AmdPowerXpressRequestHighPerformance ";
        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub static mut NvOptimusEnablement: i32 = 1;
        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub static mut AmdPowerXpressRequestHighPerformance: i32 = 1;
    };
}

/// Internal macro used for generating implementations of the `Connection` trait.
macro_rules! implement_interfaces {
    ($connection:ident) => {
        mod implementation {
            use super::connection::$connection as Connection;
            use $crate::adapter::{Adapter, AdapterPreferences};
            use $crate::connection::Connection as ConnectionInterface;
            use $crate::info::GLApi;
            use $crate::{Device, Error};

            impl ConnectionInterface for Connection {
                #[inline]
                fn new() -> Result<Connection, Error> {
                    Connection::new()
                }

                #[inline]
                fn gl_api(&self) -> GLApi {
                    Connection::gl_api(self)
                }

                #[inline]
                fn create_adapter(
                    &self,
                    preferences: AdapterPreferences,
                ) -> Result<Adapter, Error> {
                    Connection::create_adapter(self, preferences)
                }

                #[inline]
                fn create_device(&self, adapter: &Adapter) -> Result<Device, Error> {
                    Connection::create_device(self, adapter)
                }

                #[inline]
                fn from_display_handle(
                    handle: raw_window_handle::DisplayHandle,
                ) -> Result<Connection, Error> {
                    Connection::from_display_handle(handle)
                }
            }
        }
    };
}

/// A macro that takes care of producing the boilerplate for conversion to and
/// from an inner type within an enum.
macro_rules! enum_conversion {
    ($enum:ident, $variant:ident, $type:ty, $name:ident, $error:ident) => {
        impl From<$type> for $enum {
            fn from(connection: $type) -> Self {
                Self::$variant(connection)
            }
        }

        impl<'a> TryFrom<&'a mut $enum> for &'a mut $type {
            type Error = crate::Error;
            fn try_from(value: &'a mut $enum) -> Result<&'a mut $type, Self::Error> {
                #[allow(unreachable_patterns)]
                match value {
                    $enum::$variant(inner) => Ok(inner),
                    _ => Err(crate::Error::$error),
                }
            }
        }

        impl TryFrom<$enum> for $type {
            type Error = (crate::Error, $enum);
            fn try_from(value: $enum) -> Result<$type, Self::Error> {
                #[allow(unreachable_patterns)]
                match value {
                    $enum::$variant(inner) => Ok(inner),
                    _ => Err((crate::Error::$error, value)),
                }
            }
        }

        impl $enum {
            #[doc = concat!("Try to convert this generic [`", stringify!($enum), "`] into a [`", stringify!($type), "`].")]
            pub fn $name(&self) -> Result<&$type, crate::Error> {
                #[allow(unreachable_patterns)]
                match self {
                    Self::$variant(ref connection) => Ok(connection),
                    _ => Err(crate::Error::$error),
                }
            }
        }
    };
}

/// A macro that forwards a call to the backend type wrapped by a [`crate::Device`].
macro_rules! dispatch {
    ($enum:ident, $value:expr, $inner:ident => $body:expr) => {
        match $value {
            #[cfg(all(windows_platform, feature = "sm-angle"))]
            $enum::Angle($inner) => $body,
            #[cfg(macos_platform)]
            $enum::Cgl($inner) => $body,
            #[cfg(any(android_platform, ohos_platform))]
            $enum::HardwareBuffer($inner) => $body,
            #[cfg(free_unix)]
            $enum::SurfacelessMesa($inner) => $body,
            #[cfg(wayland_platform)]
            $enum::Wayland($inner) => $body,
            #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
            $enum::Wgl($inner) => $body,
            #[cfg(x11_platform)]
            $enum::X11($inner) => $body,
        }
    };
}

pub(crate) use dispatch;
pub(crate) use enum_conversion;
pub(crate) use implement_interfaces;
