// surfman/src/platform/windows/wgl/device.rs
//
//! An implementation of the GPU device for Windows using the WGL API.

use super::connection::Connection;
use super::context::WGL_EXTENSION_FUNCTIONS;
use super::surface::{Surface, Win32Objects};
use crate::context::{self, ContextID, CREATE_CONTEXT_MUTEX};
use crate::error::WindowingApiError;
use crate::renderbuffers::Renderbuffers;
use crate::surface::Framebuffer;
use crate::wgl::context::{ContextStatus, CurrentContextGuard, FramebufferGuard, OPENGL_LIBRARY};
use crate::wgl::surface::SurfaceDataGuard;
use crate::{gl, ContextDescriptor, SurfaceAccess, SurfaceType};
use crate::{gl_utils, NativeWidget};
use crate::{Context, GLApi};
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion};
use crate::{Gl, NativeContext};
use crate::{SurfaceInfo, SurfaceTexture};
use euclid::default::Size2D;
use glow::HasContext;
use libc::c_uint;
use log::warn;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use winapi::shared::dxgi::IDXGIResource;
use winapi::shared::dxgi::{IDXGIAdapter, IDXGIDevice};
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::minwindef::{self, FALSE, UINT};
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::{HBRUSH, HDC, HWND};
use winapi::shared::winerror;
use winapi::shared::winerror::S_OK;
use winapi::um::d3d11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_HARDWARE;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::libloaderapi;
use winapi::um::wingdi::{self, PIXELFORMATDESCRIPTOR};
use winapi::um::wingdi::{
    wglDeleteContext, wglGetCurrentContext, wglGetProcAddress, wglMakeCurrent,
};
use winapi::um::winuser::{self, COLOR_BACKGROUND, CS_OWNDC, MSG, WM_CLOSE};
use winapi::um::winuser::{WNDCLASSA, WS_OVERLAPPEDWINDOW};
use winapi::Interface;
use wio::com::ComPtr;

type GLenum = c_uint;
type GLint = c_int;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

const WGL_ACCESS_READ_ONLY_NV: GLenum = 0x0000;
const WGL_ACCESS_READ_WRITE_NV: GLenum = 0x0001;
const WGL_DRAW_TO_WINDOW_ARB: GLenum = 0x2001;
const WGL_ACCELERATION_ARB: GLenum = 0x2003;
const WGL_SUPPORT_OPENGL_ARB: GLenum = 0x2010;
const WGL_DOUBLE_BUFFER_ARB: GLenum = 0x2011;
const WGL_PIXEL_TYPE_ARB: GLenum = 0x2013;
const WGL_COLOR_BITS_ARB: GLenum = 0x2014;
const WGL_ALPHA_BITS_ARB: GLenum = 0x201b;
const WGL_DEPTH_BITS_ARB: GLenum = 0x2022;
const WGL_STENCIL_BITS_ARB: GLenum = 0x2023;
const WGL_FULL_ACCELERATION_ARB: GLenum = 0x2027;
const WGL_TYPE_RGBA_ARB: GLenum = 0x202b;
const WGL_CONTEXT_MAJOR_VERSION_ARB: GLenum = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: GLenum = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: GLenum = 0x9126;
const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: GLenum = 0x00000001;
const WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: GLenum = 0x00000002;

pub(crate) const HIDDEN_WINDOW_SIZE: c_int = 16;

const INTEL_PCI_ID: UINT = 0x8086;

static NVIDIA_GPU_SELECT_SYMBOL: &CStr = c"NvOptimusEnablement";
static AMD_GPU_SELECT_SYMBOL: &CStr = c"AmdPowerXpressRequestHighPerformance";

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone, Debug)]
pub enum Adapter {
    #[doc(hidden)]
    HighPerformance,
    #[doc(hidden)]
    LowPower,
}

struct SendableHWND(HWND);

unsafe impl Send for SendableHWND {}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
#[allow(dead_code)]
pub struct Device {
    pub(crate) adapter: Adapter,
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d11_device_context: ComPtr<ID3D11DeviceContext>,
    pub(crate) gl_dx_interop_device: HANDLE,
    pub(crate) hidden_window: HiddenWindow,
}

/// Wraps a Direct3D 11 device and its associated GL/DX interop device.
#[derive(Clone)]
pub struct NativeDevice {
    /// The Direct3D 11 device.
    pub d3d11_device: *mut ID3D11Device,

    /// The corresponding GL/DX interop device.
    ///
    /// The handle can be created by calling `wglDXOpenDeviceNV` from the `WGL_NV_DX_interop`
    /// extension.
    pub gl_dx_interop_device: HANDLE,
}

impl Adapter {
    pub(crate) fn set_exported_variables(&self) {
        unsafe {
            let current_module = libloaderapi::GetModuleHandleA(ptr::null());
            assert!(!current_module.is_null());
            let nvidia_gpu_select_variable: *mut i32 =
                libloaderapi::GetProcAddress(current_module, NVIDIA_GPU_SELECT_SYMBOL.as_ptr())
                    as *mut i32;
            let amd_gpu_select_variable: *mut i32 =
                libloaderapi::GetProcAddress(current_module, AMD_GPU_SELECT_SYMBOL.as_ptr())
                    as *mut i32;
            if nvidia_gpu_select_variable.is_null() || amd_gpu_select_variable.is_null() {
                println!(
                    "surfman: Could not find the NVIDIA and/or AMD GPU selection symbols. \
                       Your application may end up using the wrong GPU (discrete vs. \
                       integrated). To fix this issue, ensure that you are using the MSVC \
                       version of Rust and invoke the `declare_surfman!()` macro at the root of \
                       your crate."
                );
                warn!(
                    "surfman: Could not find the NVIDIA and/or AMD GPU selection symbols. \
                       Your application may end up using the wrong GPU (discrete vs. \
                       integrated). To fix this issue, ensure that you are using the MSVC \
                       version of Rust and invoke the `declare_surfman!()` macro at the root of \
                       your crate."
                );
                return;
            }
            let value = match *self {
                Adapter::HighPerformance => 1,
                Adapter::LowPower => 0,
            };
            *nvidia_gpu_select_variable = value;
            *amd_gpu_select_variable = value;
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS
            .dx_interop_functions
            .as_ref()
            .unwrap();
        unsafe {
            (dx_interop_functions.DXCloseDeviceNV)(self.gl_dx_interop_device);
        }
    }
}

impl Device {
    pub(crate) fn new(adapter: &Adapter) -> Result<Device, Error> {
        adapter.set_exported_variables();

        let dx_interop_functions = match WGL_EXTENSION_FUNCTIONS.dx_interop_functions {
            Some(ref dx_interop_functions) => dx_interop_functions,
            None => return Err(Error::RequiredExtensionUnavailable),
        };

        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let mut d3d11_device_context = ptr::null_mut();
            let result = D3D11CreateDevice(
                ptr::null_mut(),
                D3D_DRIVER_TYPE_HARDWARE,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                0,
                D3D11_SDK_VERSION,
                &mut d3d11_device,
                &mut d3d11_feature_level,
                &mut d3d11_device_context,
            );
            if !winerror::SUCCEEDED(result) {
                return Err(Error::DeviceOpenFailed);
            }
            let d3d11_device = ComPtr::from_raw(d3d11_device);
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let gl_dx_interop_device =
                (dx_interop_functions.DXOpenDeviceNV)(d3d11_device.as_raw() as *mut c_void);
            assert!(!gl_dx_interop_device.is_null());

            let hidden_window = HiddenWindow::new();

            Ok(Device {
                adapter: (*adapter).clone(),
                d3d11_device,
                d3d11_device_context,
                gl_dx_interop_device,
                hidden_window,
            })
        }
    }

    pub(crate) fn from_native_device(native_device: NativeDevice) -> Result<Device, Error> {
        unsafe {
            (*native_device.d3d11_device).AddRef();
            let d3d11_device = ComPtr::from_raw(native_device.d3d11_device);
            let dxgi_device: ComPtr<IDXGIDevice> = d3d11_device.cast().unwrap();

            // Fetch the DXGI adapter.
            let mut dxgi_adapter = ptr::null_mut();
            let result = dxgi_device.GetAdapter(&mut dxgi_adapter);
            assert_eq!(result, S_OK);
            assert!(!dxgi_adapter.is_null());
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            // Turn that DXGI adapter into a `surfman` adapter.
            let adapter = Adapter::from_dxgi_adapter(&dxgi_adapter);

            // Fetch the device context.
            let mut d3d11_device_context = ptr::null_mut();
            d3d11_device.GetImmediateContext(&mut d3d11_device_context);
            assert!(!d3d11_device_context.is_null());
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let gl_dx_interop_device = native_device.gl_dx_interop_device;
            let hidden_window = HiddenWindow::new();

            Ok(Device {
                adapter,
                d3d11_device,
                d3d11_device_context,
                gl_dx_interop_device,
                hidden_window,
            })
        }
    }

    /// Returns the associated Direct3D 11 device and GL/DX interop device handle.
    ///
    /// The reference count on the D3D 11 device is increased before returning.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        unsafe {
            let d3d11_device = self.d3d11_device.as_raw();
            (*d3d11_device).AddRef();
            NativeDevice {
                d3d11_device,
                gl_dx_interop_device: self.gl_dx_interop_device,
            }
        }
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    #[inline]
    pub fn adapter(&self) -> Adapter {
        self.adapter.clone()
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GL
    }

    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    #[allow(non_snake_case)]
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;
        let alpha_bits = if flags.contains(ContextAttributeFlags::ALPHA) {
            8
        } else {
            0
        };
        let depth_bits = if flags.contains(ContextAttributeFlags::DEPTH) {
            24
        } else {
            0
        };
        let stencil_bits = if flags.contains(ContextAttributeFlags::STENCIL) {
            8
        } else {
            0
        };
        let compatibility_profile = flags.contains(ContextAttributeFlags::COMPATIBILITY_PROFILE);

        let attrib_i_list = [
            WGL_DRAW_TO_WINDOW_ARB as c_int,
            gl::TRUE as c_int,
            WGL_SUPPORT_OPENGL_ARB as c_int,
            gl::TRUE as c_int,
            WGL_DOUBLE_BUFFER_ARB as c_int,
            gl::TRUE as c_int,
            WGL_PIXEL_TYPE_ARB as c_int,
            WGL_TYPE_RGBA_ARB as c_int,
            WGL_ACCELERATION_ARB as c_int,
            WGL_FULL_ACCELERATION_ARB as c_int,
            WGL_COLOR_BITS_ARB as c_int,
            32,
            WGL_ALPHA_BITS_ARB as c_int,
            alpha_bits,
            WGL_DEPTH_BITS_ARB as c_int,
            depth_bits,
            WGL_STENCIL_BITS_ARB as c_int,
            stencil_bits,
            0,
        ];

        let wglChoosePixelFormatARB = match WGL_EXTENSION_FUNCTIONS.pixel_format_functions {
            None => return Err(Error::RequiredExtensionUnavailable),
            Some(ref pixel_format_functions) => pixel_format_functions.ChoosePixelFormatARB,
        };

        let hidden_window_dc = self.hidden_window.get_dc();
        unsafe {
            let (mut pixel_format, mut pixel_format_count) = (0, 0);
            let ok = wglChoosePixelFormatARB(
                hidden_window_dc.dc,
                attrib_i_list.as_ptr(),
                ptr::null(),
                1,
                &mut pixel_format,
                &mut pixel_format_count,
            );
            if ok == FALSE {
                return Err(Error::PixelFormatSelectionFailed(WindowingApiError::Failed));
            }
            if pixel_format_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            Ok(ContextDescriptor {
                pixel_format,
                gl_version: attributes.version,
                compatibility_profile,
            })
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    #[allow(non_snake_case)]
    pub fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        let wglCreateContextAttribsARB = match WGL_EXTENSION_FUNCTIONS.CreateContextAttribsARB {
            None => return Err(Error::RequiredExtensionUnavailable),
            Some(wglCreateContextAttribsARB) => wglCreateContextAttribsARB,
        };

        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        unsafe {
            let (glrc, gl);

            // Get a suitable DC.
            let hidden_window = HiddenWindow::new();

            {
                // Set the pixel format on the hidden window DC.
                let hidden_window_dc = hidden_window.get_dc();
                let dc = hidden_window_dc.dc;
                set_dc_pixel_format(dc, descriptor.pixel_format);

                // Make the context.
                let profile_mask = if descriptor.compatibility_profile {
                    WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB
                } else {
                    WGL_CONTEXT_CORE_PROFILE_BIT_ARB
                };
                let wgl_attributes = [
                    WGL_CONTEXT_MAJOR_VERSION_ARB as c_int,
                    descriptor.gl_version.major as c_int,
                    WGL_CONTEXT_MINOR_VERSION_ARB as c_int,
                    descriptor.gl_version.minor as c_int,
                    WGL_CONTEXT_PROFILE_MASK_ARB as c_int,
                    profile_mask as c_int,
                    0,
                ];
                glrc = wglCreateContextAttribsARB(
                    dc,
                    share_with.map_or(ptr::null_mut(), |ctx| ctx.glrc),
                    wgl_attributes.as_ptr(),
                );
                if glrc.is_null() {
                    return Err(Error::ContextCreationFailed(WindowingApiError::Failed));
                }

                // Temporarily make the context current.
                let _guard = CurrentContextGuard::new();
                let ok = wglMakeCurrent(dc, glrc);
                assert_ne!(ok, FALSE);

                // Load the GL functions.
                gl = Gl::from_loader_function(get_proc_address);
            }

            // Create the initial context.
            let context = Context {
                glrc,
                id: *next_context_id,
                gl,
                hidden_window: Some(hidden_window),
                framebuffer: Framebuffer::None,
                status: ContextStatus::Owned,
            };
            next_context_id.0 += 1;
            Ok(context)
        }
    }

    /// Wraps an `HGLRC` in a `surfman` context and returns it.
    ///
    /// The `HGLRC` is not retained, as there is no way to do this in the Win32 API. Therefore, it
    /// is the caller's responsibility to make sure the OpenGL context is not destroyed before this
    /// `Context` is.
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext,
    ) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let hidden_window = HiddenWindow::new();

        // Load the GL functions.
        let gl = {
            let hidden_window_dc = hidden_window.get_dc();
            let dc = hidden_window_dc.dc;
            let _guard = CurrentContextGuard::new();
            let ok = wglMakeCurrent(dc, native_context.0);
            assert_ne!(ok, FALSE);
            Gl::from_loader_function(get_proc_address)
        };

        let context = Context {
            glrc: native_context.0,
            id: *next_context_id,
            gl,
            hidden_window: Some(hidden_window),
            framebuffer: Framebuffer::External(()),
            status: ContextStatus::Referenced,
        };
        next_context_id.0 += 1;
        Ok(context)
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.status == ContextStatus::Destroyed {
            return Ok(());
        }

        if let Ok(Some(mut surface)) = self.unbind_surface_from_context(context) {
            self.destroy_surface(context, &mut surface)?;
        }

        unsafe {
            if wglGetCurrentContext() == context.glrc {
                wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
            }

            if context.status == ContextStatus::Owned {
                wglDeleteContext(context.glrc);
            }
        }

        context.glrc = ptr::null_mut();
        context.status = ContextStatus::Destroyed;
        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            let dc_guard = self.get_context_dc(context);
            let pixel_format = wingdi::GetPixelFormat(dc_guard.dc);

            let _guard = self.temporarily_make_context_current(context);

            let gl_version = GLVersion::current(&context.gl);
            let compatibility_profile =
                context::current_context_uses_compatibility_profile(&context.gl);

            ContextDescriptor {
                pixel_format,
                gl_version,
                compatibility_profile,
            }
        }
    }

    /// Returns the attributes that the context descriptor was created with.
    #[allow(non_snake_case)]
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        let wglGetPixelFormatAttribivARB = WGL_EXTENSION_FUNCTIONS
            .pixel_format_functions
            .as_ref()
            .expect(
                "How did you make a context descriptor without \
                                            pixel format extensions?",
            )
            .GetPixelFormatAttribivARB;

        let dc_guard = self.hidden_window.get_dc();

        unsafe {
            let attrib_name_i_list = [
                WGL_ALPHA_BITS_ARB as c_int,
                WGL_DEPTH_BITS_ARB as c_int,
                WGL_STENCIL_BITS_ARB as c_int,
            ];
            let mut attrib_value_i_list = [0; 3];
            let ok = wglGetPixelFormatAttribivARB(
                dc_guard.dc,
                context_descriptor.pixel_format,
                0,
                attrib_name_i_list.len() as UINT,
                attrib_name_i_list.as_ptr(),
                attrib_value_i_list.as_mut_ptr(),
            );
            assert_ne!(ok, FALSE);
            let (alpha_bits, depth_bits, stencil_bits) = (
                attrib_value_i_list[0],
                attrib_value_i_list[1],
                attrib_value_i_list[2],
            );

            let mut attributes = ContextAttributes {
                version: context_descriptor.gl_version,
                flags: ContextAttributeFlags::empty(),
            };
            if alpha_bits > 0 {
                attributes.flags.insert(ContextAttributeFlags::ALPHA);
            }
            if depth_bits > 0 {
                attributes.flags.insert(ContextAttributeFlags::DEPTH);
            }
            if stencil_bits > 0 {
                attributes.flags.insert(ContextAttributeFlags::STENCIL);
            }

            attributes
        }
    }

    pub(crate) fn temporarily_bind_framebuffer<'a>(
        &self,
        context: &'a Context,
        framebuffer: Option<glow::Framebuffer>,
    ) -> FramebufferGuard<'a> {
        unsafe {
            let guard = FramebufferGuard::new(context);
            context.gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
            guard
        }
    }

    pub(crate) fn temporarily_make_context_current(
        &self,
        context: &Context,
    ) -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let dc_guard = self.get_context_dc(context);
            let ok = wglMakeCurrent(dc_guard.dc, context.glrc);
            if ok != FALSE {
                Ok(())
            } else {
                Err(Error::MakeCurrentFailed(WindowingApiError::Failed))
            }
        }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    #[inline]
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe {
            let ok = wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
            if ok != FALSE {
                Ok(())
            } else {
                Err(Error::MakeCurrentFailed(WindowingApiError::Failed))
            }
        }
    }

    /// Fetches the address of an OpenGL function associated with this context.
    ///
    /// OpenGL functions are local to a context. You should not use OpenGL functions on one context
    /// with any other context.
    ///
    /// This method is typically used with a function like `gl::load_with()` from the `gl` crate to
    /// load OpenGL function pointers.
    #[inline]
    pub fn get_proc_address(&self, _: &Context, symbol_name: &str) -> *const c_void {
        get_proc_address(symbol_name)
    }

    #[inline]
    fn context_is_current(&self, context: &Context) -> bool {
        unsafe { wglGetCurrentContext() == context.glrc }
    }

    /// Attaches a surface to a context for rendering.
    ///
    /// This function takes ownership of the surface. The surface must have been created with this
    /// context, or an `IncompatibleSurface` error is returned.
    ///
    /// If this function is called with a surface already bound, a `SurfaceAlreadyBound` error is
    /// returned. To avoid this error, first unbind the existing surface with
    /// `unbind_surface_from_context`.
    ///
    /// If an error is returned, the surface is returned alongside it.
    pub fn bind_surface_to_context(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<(), (Error, Surface)> {
        if context.id != surface.context_id {
            return Err((Error::IncompatibleSurface, surface));
        }

        match context.framebuffer {
            Framebuffer::None => {}
            Framebuffer::External(()) => return Err((Error::ExternalRenderTarget, surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, surface)),
        }

        let is_current = self.context_is_current(context);

        self.lock_surface(&surface);
        context.framebuffer = Framebuffer::Surface(surface);

        if is_current {
            // We need to make ourselves current again, because the surface changed.
            drop(self.make_context_current(context));
        }

        Ok(())
    }

    /// Removes and returns any attached surface from this context.
    ///
    /// Any pending OpenGL commands targeting this surface will be automatically flushed, so the
    /// surface is safe to read from immediately when this function returns.
    pub fn unbind_surface_from_context(
        &self,
        context: &mut Context,
    ) -> Result<Option<Surface>, Error> {
        match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => {
                self.unlock_surface(&surface);
                Ok(Some(surface))
            }
            Framebuffer::External(()) => Err(Error::ExternalRenderTarget),
            Framebuffer::None => Ok(None),
        }
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        match &context.framebuffer {
            Framebuffer::Surface(surface) => surface.present(),
            _ => Ok(()),
        }
    }

    /// If the currently bound surface is a widget surface, resize it,
    pub fn resize_bound_surface(
        &self,
        context: &mut Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        if let Framebuffer::Surface(surface) = &mut context.framebuffer {
            surface.resize(size);
        }
        Ok(())
    }

    pub(crate) fn get_context_dc<'a>(&self, context: &'a Context) -> DCGuard<'a> {
        unsafe {
            match context.framebuffer {
                Framebuffer::Surface(Surface {
                    win32_objects: Win32Objects::Widget { window_handle },
                    ..
                }) => DCGuard::new(winuser::GetDC(window_handle), Some(window_handle)),
                Framebuffer::Surface(Surface {
                    win32_objects: Win32Objects::Texture { .. },
                    ..
                })
                | Framebuffer::External(())
                | Framebuffer::None => context.hidden_window.as_ref().unwrap().get_dc(),
            }
        }
    }

    /// Returns a unique ID representing a context.
    ///
    /// This ID is unique to all currently-allocated contexts. If you destroy a context and create
    /// a new one, the new context might have the same ID as the destroyed one.
    #[inline]
    pub fn context_id(&self, context: &Context) -> ContextID {
        context.id
    }

    /// Returns various information about the surface attached to a context.
    ///
    /// This includes, most notably, the OpenGL framebuffer object needed to render to the surface.
    pub fn context_surface_info(&self, context: &Context) -> Result<Option<SurfaceInfo>, Error> {
        match context.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External(()) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }

    /// Given a context, returns its underlying `HGLRC`.
    #[inline]
    pub fn native_context(&self, context: &Context) -> NativeContext {
        NativeContext(context.glrc)
    }

    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &self,
        context: &Context,
        _: SurfaceAccess,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, Error> {
        match surface_type {
            SurfaceType::Generic { size } => self.create_generic_surface(context, &size),
            SurfaceType::Widget { native_widget } => {
                self.create_widget_surface(context, native_widget)
            }
        }
    }

    fn create_generic_surface(
        &self,
        context: &Context,
        size: &Size2D<i32>,
    ) -> Result<Surface, Error> {
        let dx_interop_functions = match WGL_EXTENSION_FUNCTIONS.dx_interop_functions {
            None => return Err(Error::RequiredExtensionUnavailable),
            Some(ref dx_interop_functions) => dx_interop_functions,
        };

        unsafe {
            let _guard = self.temporarily_make_context_current(context)?;

            // Create the Direct3D 11 texture.
            let d3d11_texture2d_desc = D3D11_TEXTURE2D_DESC {
                Width: size.width as UINT,
                Height: size.height as UINT,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET,
                CPUAccessFlags: 0,
                MiscFlags: D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX,
            };
            let mut d3d11_texture = ptr::null_mut();
            let mut result = self.d3d11_device.CreateTexture2D(
                &d3d11_texture2d_desc,
                ptr::null(),
                &mut d3d11_texture,
            );
            if !winerror::SUCCEEDED(result) {
                return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
            }
            assert!(!d3d11_texture.is_null());
            let d3d11_texture = ComPtr::from_raw(d3d11_texture);

            // Upcast it to a DXGI resource.
            let mut dxgi_resource: *mut IDXGIResource = ptr::null_mut();
            result = d3d11_texture.QueryInterface(
                &IDXGIResource::uuidof(),
                &mut dxgi_resource as *mut *mut IDXGIResource as *mut *mut c_void,
            );
            assert!(winerror::SUCCEEDED(result));
            assert!(!dxgi_resource.is_null());
            let dxgi_resource = ComPtr::from_raw(dxgi_resource);

            // Get the share handle. We'll need it both to bind to GL and to share the texture
            // across contexts.
            let mut dxgi_share_handle = INVALID_HANDLE_VALUE;
            result = dxgi_resource.GetSharedHandle(&mut dxgi_share_handle);
            assert!(winerror::SUCCEEDED(result));
            assert_ne!(dxgi_share_handle, INVALID_HANDLE_VALUE);

            // Tell GL about the share handle.
            let ok = (dx_interop_functions.DXSetResourceShareHandleNV)(
                d3d11_texture.as_raw() as *mut c_void,
                dxgi_share_handle,
            );
            assert_ne!(ok, FALSE);

            // Make our texture object on the GL side.
            let gl_texture = context.gl.create_texture().unwrap();

            // Bind the GL texture to the D3D11 texture.
            let gl_dx_interop_object = (dx_interop_functions.DXRegisterObjectNV)(
                self.gl_dx_interop_device,
                d3d11_texture.as_raw() as *mut c_void,
                gl_texture.0.get(),
                gl::TEXTURE_2D,
                WGL_ACCESS_READ_WRITE_NV,
            );
            // Per the spec, and unlike other HANDLEs, null indicates an error.
            if gl_dx_interop_object.is_null() {
                let msg = std::io::Error::last_os_error(); // Equivalent to GetLastError().
                error!(
                    "Unable to share surface between OpenGL and DirectX. OS error '{}'.",
                    msg
                );
                return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
            }

            // Build our FBO.
            let gl_framebuffer = context.gl.create_framebuffer().unwrap();
            let _guard = self.temporarily_bind_framebuffer(context, Some(gl_framebuffer));

            // Attach the reflected D3D11 texture to that FBO.
            context.gl.framebuffer_texture_2d(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                SURFACE_GL_TEXTURE_TARGET,
                Some(gl_texture),
                0,
            );

            // Create renderbuffers as appropriate, and attach them.
            let context_descriptor = self.context_descriptor(context);
            let context_attributes = self.context_descriptor_attributes(&context_descriptor);
            let renderbuffers = Renderbuffers::new(&context.gl, &size, &context_attributes);
            renderbuffers.bind_to_current_framebuffer(&context.gl);

            // FIXME(pcwalton): Do we need to acquire the keyed mutex, or does the GL driver do
            // that?

            Ok(Surface {
                size: *size,
                context_id: context.id,
                win32_objects: Win32Objects::Texture {
                    d3d11_texture,
                    dxgi_share_handle,
                    gl_dx_interop_object,
                    gl_texture: Some(gl_texture),
                    gl_framebuffer: Some(gl_framebuffer),
                    renderbuffers,
                },
                destroyed: false,
            })
        }
    }

    fn create_widget_surface(
        &self,
        context: &Context,
        native_widget: NativeWidget,
    ) -> Result<Surface, Error> {
        unsafe {
            // Get the bounds of the native HWND.
            let mut widget_rect = mem::zeroed();
            let ok = winuser::GetWindowRect(native_widget.window_handle, &mut widget_rect);
            if ok == FALSE {
                return Err(Error::InvalidNativeWidget);
            }

            // Set its pixel format.
            {
                let context_dc_guard = self.get_context_dc(context);
                let pixel_format = wingdi::GetPixelFormat(context_dc_guard.dc);
                let window_dc = winuser::GetDC(native_widget.window_handle);
                set_dc_pixel_format(window_dc, pixel_format);
            }

            Ok(Surface {
                size: Size2D::new(
                    widget_rect.right - widget_rect.left,
                    widget_rect.bottom - widget_rect.top,
                ),
                context_id: context.id,
                win32_objects: Win32Objects::Widget {
                    window_handle: native_widget.window_handle,
                },
                destroyed: false,
            })
        }
    }

    /// Destroys a surface.
    ///
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    ///
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(
        &self,
        context: &mut Context,
        surface: &mut Surface,
    ) -> Result<(), Error> {
        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS
            .dx_interop_functions
            .as_ref()
            .expect("How did you make a surface without DX interop?");

        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        let _guard = self.temporarily_make_context_current(context)?;

        unsafe {
            match surface.win32_objects {
                Win32Objects::Texture {
                    ref mut gl_dx_interop_object,
                    ref mut gl_texture,
                    ref mut gl_framebuffer,
                    ref mut renderbuffers,
                    d3d11_texture: _,
                    dxgi_share_handle: _,
                } => {
                    renderbuffers.destroy(&context.gl);

                    if let Some(fbo) = gl_framebuffer.take() {
                        gl_utils::destroy_framebuffer(&context.gl, fbo);
                    }

                    if let Some(texture) = gl_texture.take() {
                        context.gl.delete_texture(texture);
                    }

                    let ok = (dx_interop_functions.DXUnregisterObjectNV)(
                        self.gl_dx_interop_device,
                        *gl_dx_interop_object,
                    );
                    assert_ne!(ok, FALSE);
                    *gl_dx_interop_object = INVALID_HANDLE_VALUE;
                }
                Win32Objects::Widget { window_handle: _ } => {}
            }

            surface.destroyed = true;
        }

        Ok(())
    }

    /// Creates a surface texture from an existing generic surface for use with the given context.
    ///
    /// The surface texture is local to the supplied context and takes ownership of the surface.
    /// Destroying the surface texture allows you to retrieve the surface again.
    ///
    /// *The supplied context does not have to be the same context that the surface is associated
    /// with.* This allows you to render to a surface in one context and sample from that surface
    /// in another context.
    ///
    /// Calling this method on a widget surface returns a `WidgetAttached` error.
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        let dxgi_share_handle = match surface.win32_objects {
            Win32Objects::Widget { .. } => return Err((Error::WidgetAttached, surface)),
            Win32Objects::Texture {
                dxgi_share_handle, ..
            } => dxgi_share_handle,
        };

        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS
            .dx_interop_functions
            .as_ref()
            .expect("How did you make a surface without DX interop?");

        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(err) => return Err((err, surface)),
        };

        unsafe {
            // Create a new texture wrapping the shared handle.
            let mut local_d3d11_texture = ptr::null_mut();
            let result = self.d3d11_device.OpenSharedResource(
                dxgi_share_handle,
                &ID3D11Texture2D::uuidof(),
                &mut local_d3d11_texture,
            );
            if !winerror::SUCCEEDED(result) || local_d3d11_texture.is_null() {
                return Err((
                    Error::SurfaceImportFailed(WindowingApiError::Failed),
                    surface,
                ));
            }
            let local_d3d11_texture = ComPtr::from_raw(local_d3d11_texture as *mut ID3D11Texture2D);

            // Make GL aware of the connection between the share handle and the texture.
            let ok = (dx_interop_functions.DXSetResourceShareHandleNV)(
                local_d3d11_texture.as_raw() as *mut c_void,
                dxgi_share_handle,
            );
            assert_ne!(ok, FALSE);

            // Create a GL texture.
            let gl_texture = context.gl.create_texture().unwrap();

            // Register that texture with GL/DX interop.
            let mut local_gl_dx_interop_object = (dx_interop_functions.DXRegisterObjectNV)(
                self.gl_dx_interop_device,
                local_d3d11_texture.as_raw() as *mut c_void,
                gl_texture.0.get(),
                gl::TEXTURE_2D,
                WGL_ACCESS_READ_ONLY_NV,
            );

            // Lock the texture so that we can use it.
            let ok = (dx_interop_functions.DXLockObjectsNV)(
                self.gl_dx_interop_device,
                1,
                &mut local_gl_dx_interop_object,
            );
            assert_ne!(ok, FALSE);

            // Initialize the texture, for convenience.
            // FIXME(pcwalton): We should probably reset the bound texture after this.
            context.gl.bind_texture(gl::TEXTURE_2D, Some(gl_texture));
            context.gl.tex_parameter_i32(
                gl::TEXTURE_2D,
                gl::TEXTURE_MAG_FILTER,
                gl::LINEAR as GLint,
            );
            context.gl.tex_parameter_i32(
                gl::TEXTURE_2D,
                gl::TEXTURE_MIN_FILTER,
                gl::LINEAR as GLint,
            );
            context.gl.tex_parameter_i32(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            context.gl.tex_parameter_i32(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );

            // Finish up.
            Ok(SurfaceTexture {
                surface,
                local_d3d11_texture,
                local_gl_dx_interop_object,
                gl_texture: Some(gl_texture),
                phantom: PhantomData,
            })
        }
    }

    /// Destroys a surface texture and returns the underlying surface.
    ///
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    ///
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context,
        mut surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS
            .dx_interop_functions
            .as_ref()
            .expect("How did you make a surface without DX interop?");

        let _guard = match self.temporarily_make_context_current(context) {
            Ok(guard) => guard,
            Err(err) => return Err((err, surface_texture)),
        };

        unsafe {
            // Unlock the texture.
            let ok = (dx_interop_functions.DXUnlockObjectsNV)(
                self.gl_dx_interop_device,
                1,
                &mut surface_texture.local_gl_dx_interop_object,
            );
            assert_ne!(ok, FALSE);

            // Unregister the texture from GL/DX interop.
            let ok = (dx_interop_functions.DXUnregisterObjectNV)(
                self.gl_dx_interop_device,
                surface_texture.local_gl_dx_interop_object,
            );
            assert_ne!(ok, FALSE);
            surface_texture.local_gl_dx_interop_object = INVALID_HANDLE_VALUE;

            // Destroy the GL texture.
            if let Some(texture) = surface_texture.gl_texture.take() {
                context.gl.delete_texture(texture);
            }
        }

        Ok(surface_texture.surface)
    }

    pub(crate) fn lock_surface(&self, surface: &Surface) {
        let mut gl_dx_interop_object = match surface.win32_objects {
            Win32Objects::Widget { .. } => return,
            Win32Objects::Texture {
                gl_dx_interop_object,
                ..
            } => gl_dx_interop_object,
        };

        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS
            .dx_interop_functions
            .as_ref()
            .expect("How did you make a surface without DX interop?");

        unsafe {
            let ok = (dx_interop_functions.DXLockObjectsNV)(
                self.gl_dx_interop_device,
                1,
                &mut gl_dx_interop_object,
            );
            assert_ne!(ok, FALSE);
        }
    }

    pub(crate) fn unlock_surface(&self, surface: &Surface) {
        let mut gl_dx_interop_object = match surface.win32_objects {
            Win32Objects::Widget { .. } => return,
            Win32Objects::Texture {
                gl_dx_interop_object,
                ..
            } => gl_dx_interop_object,
        };

        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS
            .dx_interop_functions
            .as_ref()
            .expect("How did you make a surface without DX interop?");

        unsafe {
            let ok = (dx_interop_functions.DXUnlockObjectsNV)(
                self.gl_dx_interop_device,
                1,
                &mut gl_dx_interop_object,
            );
            assert_ne!(ok, FALSE);
        }
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(
        &self,
        _surface: &'s mut Surface,
    ) -> Result<SurfaceDataGuard<'s>, Error> {
        Err(Error::Unimplemented)
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        gl::TEXTURE_2D
    }

    /// Displays the contents of a widget surface on screen.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    ///
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        surface.present()
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        _scontext: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        surface.resize(size);
        Ok(())
    }

    /// Returns various information about the surface, including the framebuffer object needed to
    /// render to this surface.
    ///
    /// Before rendering to a surface attached to a context, you must call `glBindFramebuffer()`
    /// on the framebuffer object returned by this function. This framebuffer object may or not be
    /// 0, the default framebuffer, depending on platform.
    #[inline]
    pub fn surface_info(&self, surface: &Surface) -> SurfaceInfo {
        SurfaceInfo {
            size: surface.size,
            id: surface.id(),
            context_id: surface.context_id,
            framebuffer_object: match surface.win32_objects {
                Win32Objects::Texture { gl_framebuffer, .. } => gl_framebuffer,
                Win32Objects::Widget { .. } => None,
            },
        }
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(
        &self,
        surface_texture: &SurfaceTexture,
    ) -> Option<glow::Texture> {
        surface_texture.gl_texture
    }
}

impl Adapter {
    fn from_dxgi_adapter(dxgi_adapter: &ComPtr<IDXGIAdapter>) -> Adapter {
        unsafe {
            let mut adapter_desc = mem::zeroed();
            let result = dxgi_adapter.GetDesc(&mut adapter_desc);
            assert_eq!(result, S_OK);

            if adapter_desc.VendorId == INTEL_PCI_ID {
                Adapter::LowPower
            } else {
                Adapter::HighPerformance
            }
        }
    }
}

pub(crate) struct HiddenWindow {
    window: HWND,
    join_handle: Option<JoinHandle<()>>,
}

pub(crate) struct DCGuard<'a> {
    pub(crate) dc: HDC,
    window: Option<HWND>,
    phantom: PhantomData<&'a HWND>,
}

impl Drop for HiddenWindow {
    fn drop(&mut self) {
        unsafe {
            winuser::PostMessageA(self.window, WM_CLOSE, 0, 0);
            if let Some(join_handle) = self.join_handle.take() {
                drop(join_handle.join());
            }
        }
    }
}

impl<'a> Drop for DCGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if let Some(window) = self.window {
                winuser::ReleaseDC(window, self.dc);
            }
        }
    }
}

impl HiddenWindow {
    pub(crate) fn new() -> HiddenWindow {
        let (sender, receiver) = mpsc::channel();
        let join_handle = thread::spawn(|| HiddenWindow::thread(sender));
        let window = receiver.recv().unwrap().0;
        HiddenWindow {
            window,
            join_handle: Some(join_handle),
        }
    }

    #[inline]
    pub(crate) fn get_dc(&self) -> DCGuard<'_> {
        unsafe { DCGuard::new(winuser::GetDC(self.window), Some(self.window)) }
    }

    // The thread that creates the window for off-screen contexts.
    fn thread(sender: Sender<SendableHWND>) {
        unsafe {
            let instance = libloaderapi::GetModuleHandleA(ptr::null_mut());
            let window_class_name = c"SurfmanHiddenWindow".as_ptr();
            let mut window_class = mem::zeroed();
            if winuser::GetClassInfoA(instance, window_class_name, &mut window_class) == FALSE {
                window_class = WNDCLASSA {
                    style: CS_OWNDC,
                    lpfnWndProc: Some(winuser::DefWindowProcA),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: instance,
                    hIcon: ptr::null_mut(),
                    hCursor: ptr::null_mut(),
                    hbrBackground: COLOR_BACKGROUND as HBRUSH,
                    lpszMenuName: ptr::null_mut(),
                    lpszClassName: window_class_name,
                };
                let window_class_atom = winuser::RegisterClassA(&window_class);
                assert_ne!(window_class_atom, 0);
            }

            let window = winuser::CreateWindowExA(
                0,
                window_class_name,
                window_class_name,
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                HIDDEN_WINDOW_SIZE,
                HIDDEN_WINDOW_SIZE,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );

            sender.send(SendableHWND(window)).unwrap();

            let mut msg: MSG = mem::zeroed();
            while winuser::GetMessageA(&mut msg, window, 0, 0) != FALSE {
                winuser::TranslateMessage(&msg);
                winuser::DispatchMessageA(&msg);
                if minwindef::LOWORD(msg.message) as UINT == WM_CLOSE {
                    break;
                }
            }
        }
    }
}

impl<'a> DCGuard<'a> {
    pub(crate) fn new(dc: HDC, window: Option<HWND>) -> DCGuard<'a> {
        DCGuard {
            dc,
            window,
            phantom: PhantomData,
        }
    }
}

fn get_proc_address(symbol_name: &str) -> *const c_void {
    unsafe {
        // https://www.khronos.org/opengl/wiki/Load_OpenGL_Functions#Windows
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        let symbol_ptr = symbol_name.as_ptr();
        let addr = wglGetProcAddress(symbol_ptr) as *const c_void;
        if !addr.is_null() {
            return addr;
        }
        OPENGL_LIBRARY.with(|opengl_library| {
            libloaderapi::GetProcAddress(*opengl_library, symbol_ptr) as *const c_void
        })
    }
}

pub(crate) fn set_dc_pixel_format(dc: HDC, pixel_format: c_int) {
    unsafe {
        let mut pixel_format_descriptor = mem::zeroed();
        let pixel_format_count = wingdi::DescribePixelFormat(
            dc,
            pixel_format,
            mem::size_of::<PIXELFORMATDESCRIPTOR>() as UINT,
            &mut pixel_format_descriptor,
        );
        assert_ne!(pixel_format_count, 0);
        let ok = wingdi::SetPixelFormat(dc, pixel_format, &mut pixel_format_descriptor);
        assert_ne!(ok, FALSE);
    }
}
