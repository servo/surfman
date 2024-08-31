// surfman/surfman/src/macros.rs
//
//! A macro for use in the top-level crate.

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

/// Internal macro used for generating implementations of the `Connection` and `Device` traits.
macro_rules! implement_interfaces {
    () => {
        mod implementation {
            use super::connection::{Connection, NativeConnection};
            use super::context::{Context, ContextDescriptor, NativeContext};
            use super::device::{Adapter, Device, NativeDevice};
            use super::surface::{NativeWidget, Surface, SurfaceTexture};
            use euclid::default::Size2D;
            use std::os::raw::c_void;
            use $crate::connection::Connection as ConnectionInterface;
            use $crate::device::Device as DeviceInterface;
            use $crate::gl::types::{GLenum, GLuint};
            use $crate::info::GLApi;
            use $crate::Error;
            use $crate::{ContextAttributes, ContextID, SurfaceAccess, SurfaceInfo, SurfaceType};

            impl ConnectionInterface for Connection {
                type Adapter = Adapter;
                type Device = Device;
                type NativeConnection = NativeConnection;
                type NativeDevice = NativeDevice;
                type NativeWidget = NativeWidget;

                #[inline]
                fn new() -> Result<Connection, Error> {
                    Connection::new()
                }

                #[inline]
                fn native_connection(&self) -> Self::NativeConnection {
                    Connection::native_connection(self)
                }

                #[inline]
                fn gl_api(&self) -> GLApi {
                    Connection::gl_api(self)
                }

                #[inline]
                fn create_adapter(&self) -> Result<Self::Adapter, Error> {
                    Connection::create_adapter(self)
                }

                #[inline]
                fn create_hardware_adapter(&self) -> Result<Self::Adapter, Error> {
                    Connection::create_hardware_adapter(self)
                }

                #[inline]
                fn create_low_power_adapter(&self) -> Result<Self::Adapter, Error> {
                    Connection::create_low_power_adapter(self)
                }

                #[inline]
                fn create_software_adapter(&self) -> Result<Self::Adapter, Error> {
                    Connection::create_software_adapter(self)
                }

                #[inline]
                fn create_device(&self, adapter: &Adapter) -> Result<Self::Device, Error> {
                    Connection::create_device(self, adapter)
                }

                #[inline]
                unsafe fn create_device_from_native_device(
                    &self,
                    native_device: Self::NativeDevice,
                ) -> Result<Device, Error> {
                    Connection::create_device_from_native_device(self, native_device)
                }

                #[inline]
                #[cfg(feature = "sm-raw-window-handle-05")]
                fn from_raw_display_handle(
                    raw_handle: rwh_05::RawDisplayHandle,
                ) -> Result<Connection, Error> {
                    Connection::from_raw_display_handle(raw_handle)
                }

                #[inline]
                #[cfg(feature = "sm-raw-window-handle-06")]
                fn from_display_handle(handle: rwh_06::DisplayHandle) -> Result<Connection, Error> {
                    Connection::from_display_handle(handle)
                }

                #[inline]
                unsafe fn create_native_widget_from_ptr(
                    &self,
                    raw: *mut c_void,
                    size: Size2D<i32>,
                ) -> Self::NativeWidget {
                    Connection::create_native_widget_from_ptr(self, raw, size)
                }

                #[inline]
                #[cfg(feature = "sm-raw-window-handle-05")]
                fn create_native_widget_from_raw_window_handle(
                    &self,
                    window: rwh_05::RawWindowHandle,
                    size: Size2D<i32>,
                ) -> Result<Self::NativeWidget, Error> {
                    Connection::create_native_widget_from_raw_window_handle(self, window, size)
                }

                #[inline]
                #[cfg(feature = "sm-raw-window-handle-06")]
                fn create_native_widget_from_window_handle(
                    &self,
                    window: rwh_06::WindowHandle,
                    size: Size2D<i32>,
                ) -> Result<Self::NativeWidget, Error> {
                    Connection::create_native_widget_from_window_handle(self, window, size)
                }
            }

            impl DeviceInterface for Device {
                type Connection = Connection;
                type Context = Context;
                type ContextDescriptor = ContextDescriptor;
                type NativeContext = NativeContext;
                type Surface = Surface;
                type SurfaceTexture = SurfaceTexture;

                // device.rs

                /// Returns the native device associated with this device.
                #[inline]
                fn native_device(&self) -> <Self::Connection as ConnectionInterface>::NativeDevice {
                    Device::native_device(self)
                }

                #[inline]
                fn connection(&self) -> Connection {
                    Device::connection(self)
                }

                #[inline]
                fn adapter(&self) -> Adapter {
                    Device::adapter(self)
                }

                #[inline]
                fn gl_api(&self) -> GLApi {
                    Device::gl_api(self)
                }

                // context.rs

                #[inline]
                fn create_context_descriptor(
                    &self,
                    attributes: &ContextAttributes,
                ) -> Result<Self::ContextDescriptor, Error> {
                    Device::create_context_descriptor(self, attributes)
                }

                #[inline]
                fn create_context(
                    &mut self,
                    descriptor: &Self::ContextDescriptor,
                    share_with: Option<&Self::Context>,
                ) -> Result<Self::Context, Error> {
                    Device::create_context(self, descriptor, share_with)
                }

                #[inline]
                unsafe fn create_context_from_native_context(
                    &self,
                    native_context: Self::NativeContext,
                ) -> Result<Self::Context, Error> {
                    Device::create_context_from_native_context(self, native_context)
                }

                #[inline]
                fn destroy_context(&self, context: &mut Self::Context) -> Result<(), Error> {
                    Device::destroy_context(self, context)
                }

                #[inline]
                fn context_descriptor(&self, context: &Self::Context) -> Self::ContextDescriptor {
                    Device::context_descriptor(self, context)
                }

                #[inline]
                fn make_context_current(&self, context: &Self::Context) -> Result<(), Error> {
                    Device::make_context_current(self, context)
                }

                #[inline]
                fn make_no_context_current(&self) -> Result<(), Error> {
                    Device::make_no_context_current(self)
                }

                #[inline]
                fn context_descriptor_attributes(
                    &self,
                    context_descriptor: &Self::ContextDescriptor,
                ) -> ContextAttributes {
                    Device::context_descriptor_attributes(self, context_descriptor)
                }

                #[inline]
                fn get_proc_address(
                    &self,
                    context: &Self::Context,
                    symbol_name: &str,
                ) -> *const c_void {
                    Device::get_proc_address(self, context, symbol_name)
                }

                #[inline]
                fn bind_surface_to_context(
                    &self,
                    context: &mut Self::Context,
                    surface: Self::Surface,
                ) -> Result<(), (Error, Self::Surface)> {
                    Device::bind_surface_to_context(self, context, surface)
                }

                #[inline]
                fn unbind_surface_from_context(
                    &self,
                    context: &mut Self::Context,
                ) -> Result<Option<Self::Surface>, Error> {
                    Device::unbind_surface_from_context(self, context)
                }

                #[inline]
                fn context_id(&self, context: &Self::Context) -> ContextID {
                    Device::context_id(self, context)
                }

                #[inline]
                fn context_surface_info(
                    &self,
                    context: &Self::Context,
                ) -> Result<Option<SurfaceInfo>, Error> {
                    Device::context_surface_info(self, context)
                }

                #[inline]
                fn native_context(&self, context: &Self::Context) -> Self::NativeContext {
                    Device::native_context(self, context)
                }

                // surface.rs

                #[inline]
                fn create_surface(
                    &mut self,
                    context: &Self::Context,
                    surface_access: SurfaceAccess,
                    surface_type: SurfaceType<NativeWidget>,
                ) -> Result<Self::Surface, Error> {
                    Device::create_surface(self, context, surface_access, surface_type)
                }

                #[inline]
                fn create_surface_texture(
                    &self,
                    context: &mut Self::Context,
                    surface: Self::Surface,
                ) -> Result<Self::SurfaceTexture, (Error, Self::Surface)> {
                    Device::create_surface_texture(self, context, surface)
                }

                #[inline]
                fn destroy_surface(
                    &self,
                    context: &mut Self::Context,
                    surface: &mut Self::Surface,
                ) -> Result<(), Error> {
                    Device::destroy_surface(self, context, surface)
                }

                #[inline]
                fn destroy_surface_texture(
                    &self,
                    context: &mut Self::Context,
                    surface_texture: Self::SurfaceTexture,
                ) -> Result<Self::Surface, (Error, Self::SurfaceTexture)> {
                    Device::destroy_surface_texture(self, context, surface_texture)
                }

                #[inline]
                fn surface_gl_texture_target(&self) -> GLenum {
                    Device::surface_gl_texture_target(self)
                }

                #[inline]
                fn present_surface(
                    &self,
                    context: &Self::Context,
                    surface: &mut Self::Surface,
                ) -> Result<(), Error> {
                    Device::present_surface(self, context, surface)
                }

                #[inline]
                fn resize_surface(
                    &self,
                    context: &Context,
                    surface: &mut Surface,
                    size: Size2D<i32>,
                ) -> Result<(), Error> {
                    Device::resize_surface(self, context, surface, size)
                }

                #[inline]
                fn surface_info(&self, surface: &Self::Surface) -> SurfaceInfo {
                    Device::surface_info(self, surface)
                }

                #[inline]
                fn surface_texture_object(&self, surface_texture: &Self::SurfaceTexture) -> GLuint {
                    Device::surface_texture_object(self, surface_texture)
                }
            }
        }
    };
}

pub(crate) use implement_interfaces;
