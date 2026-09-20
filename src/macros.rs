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
            use super::connection::Connection;
            use super::context::{Context, ContextDescriptor};
            use super::device::Device;
            use super::surface::{NativeWidget, Surface, SurfaceTexture};
            use euclid::default::Size2D;
            use glow::Texture;
            use std::os::raw::c_void;
            use $crate::adapter::{Adapter, AdapterPreferences};
            use $crate::connection::Connection as ConnectionInterface;
            use $crate::device::Device as DeviceInterface;
            use $crate::info::GLApi;
            use $crate::Error;
            use $crate::{ContextAttributes, ContextID, SurfaceAccess, SurfaceInfo, SurfaceType};

            impl ConnectionInterface for Connection {
                type Device = Device;
                type NativeWidget = NativeWidget;

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
                fn create_device(&self, adapter: &Adapter) -> Result<Self::Device, Error> {
                    Connection::create_device(self, adapter)
                }

                #[inline]
                fn from_display_handle(
                    handle: raw_window_handle::DisplayHandle,
                ) -> Result<Connection, Error> {
                    Connection::from_display_handle(handle)
                }

                #[inline]
                fn create_native_widget_from_window_handle(
                    &self,
                    window: raw_window_handle::WindowHandle,
                    size: Size2D<i32>,
                ) -> Result<Self::NativeWidget, Error> {
                    Connection::create_native_widget_from_window_handle(self, window, size)
                }
            }

            impl DeviceInterface for Device {
                type Connection = Connection;
                type Context = Context;
                type ContextDescriptor = ContextDescriptor;
                type Surface = Surface;
                type SurfaceTexture = SurfaceTexture;

                // device.rs

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
                    &self,
                    descriptor: &Self::ContextDescriptor,
                    share_with: Option<&Self::Context>,
                ) -> Result<Self::Context, Error> {
                    Device::create_context(self, descriptor, share_with)
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

                // surface.rs

                #[inline]
                fn create_surface(
                    &self,
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
                fn surface_gl_texture_target(&self) -> u32 {
                    Device::surface_gl_texture_target(self)
                }

                #[inline]
                fn present_bound_surface(&self, context: &mut Self::Context) -> Result<(), Error> {
                    Device::present_bound_surface(self, context)
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
                fn resize_bound_surface(
                    &self,
                    context: &mut Context,
                    size: Size2D<i32>,
                ) -> Result<(), Error> {
                    Device::resize_bound_surface(self, context, size)
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
                fn surface_texture_object(
                    &self,
                    surface_texture: &Self::SurfaceTexture,
                ) -> Option<Texture> {
                    Device::surface_texture_object(self, surface_texture)
                }
            }
        }
    };
}

pub(crate) use implement_interfaces;
