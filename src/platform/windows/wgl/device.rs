// surfman/src/platform/windows/wgl/device.rs
//
//! An implementation of the GPU device for Windows using the WGL API.

use super::connection::Connection;
use super::context::WGL_EXTENSION_FUNCTIONS;
use crate::{Error, GLApi};

use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use windows::core::{Interface, PCSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HMODULE, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_FLAG,
    D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};
use windows::Win32::Graphics::Gdi::ReleaseDC;
use windows::Win32::Graphics::Gdi::COLOR_BACKGROUND;
use windows::Win32::Graphics::Gdi::{GetDC, HBRUSH, HDC};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExA, DefWindowProcA, DispatchMessageA, GetClassInfoA, GetMessageA, PostMessageA,
    RegisterClassA, TranslateMessage, HCURSOR, HICON, HMENU, WINDOW_EX_STYLE,
};
use windows::Win32::UI::WindowsAndMessaging::{CS_OWNDC, MSG, WM_CLOSE};
use windows::Win32::UI::WindowsAndMessaging::{WNDCLASSA, WS_OVERLAPPEDWINDOW};

pub(crate) const HIDDEN_WINDOW_SIZE: c_int = 16;

const INTEL_PCI_ID: u32 = 0x8086;

static NVIDIA_GPU_SELECT_SYMBOL: &[u8] = b"NvOptimusEnablement\0";
static AMD_GPU_SELECT_SYMBOL: &[u8] = b"AmdPowerXpressRequestHighPerformance\0";

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
    pub(crate) d3d11_device: ID3D11Device,
    pub(crate) d3d11_device_context: ID3D11DeviceContext,
    pub(crate) gl_dx_interop_device: HANDLE,
    pub(crate) hidden_window: HiddenWindow,
}

/// Wraps a Direct3D 11 device and its associated GL/DX interop device.
#[derive(Clone)]
pub struct NativeDevice {
    /// The Direct3D 11 device.
    pub d3d11_device: ID3D11Device,

    /// The corresponding GL/DX interop device.
    ///
    /// The handle can be created by calling `wglDXOpenDeviceNV` from the `WGL_NV_DX_interop`
    /// extension.
    pub gl_dx_interop_device: HANDLE,
}

impl Adapter {
    pub(crate) fn set_exported_variables(&self) {
        unsafe {
            let current_module = GetModuleHandleA(PCSTR::null());
            assert!(current_module.is_ok());
            let current_module = current_module.unwrap();
            let nvidia_gpu_select_variable =
                GetProcAddress(current_module, PCSTR(NVIDIA_GPU_SELECT_SYMBOL.as_ptr()));
            let amd_gpu_select_variable =
                GetProcAddress(current_module, PCSTR(AMD_GPU_SELECT_SYMBOL.as_ptr()));
            if nvidia_gpu_select_variable.is_none() || amd_gpu_select_variable.is_none() {
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
            if nvidia_gpu_select_variable.is_some() {
                let nvidia_gpu_select_variable = nvidia_gpu_select_variable.unwrap();
                *(nvidia_gpu_select_variable as *mut i32) = value;
            }
            if amd_gpu_select_variable.is_some() {
                let amd_gpu_select_variable = amd_gpu_select_variable.unwrap();
                *(amd_gpu_select_variable as *mut i32) = value;
            }
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
            let mut d3d11_device = Default::default();
            let mut d3d11_feature_level = ptr::null_mut();
            let mut d3d11_device_context = Default::default();
            let result = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d11_device),
                Some(d3d11_feature_level),
                Some(&mut d3d11_device_context),
            );
            if result.is_err() {
                return Err(Error::DeviceOpenFailed);
            }
            let d3d11_device = d3d11_device.unwrap();
            let d3d11_device_context = d3d11_device_context.unwrap();

            let gl_dx_interop_device =
                (dx_interop_functions.DXOpenDeviceNV)(d3d11_device.as_raw() as *mut c_void);
            assert!(!gl_dx_interop_device.is_invalid());

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
            let d3d11_device = native_device.d3d11_device;
            let dxgi_device = d3d11_device.cast::<IDXGIDevice>();

            assert!(dxgi_device.is_ok());

            // Fetch the DXGI adapter.
            let result = dxgi_device.unwrap().GetAdapter();
            assert!(result.is_ok());
            let dxgi_adapter = result.unwrap();

            // Turn that DXGI adapter into a `surfman` adapter.
            let adapter = Adapter::from_dxgi_adapter(&dxgi_adapter);

            // Fetch the device context.
            let mut d3d11_device_context = d3d11_device.GetImmediateContext();
            assert!(d3d11_device_context.is_ok());
            let d3d11_device_context = d3d11_device_context.unwrap();

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
            let d3d11_device = self.d3d11_device.clone();
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
}

impl Adapter {
    fn from_dxgi_adapter(dxgi_adapter: &IDXGIAdapter) -> Adapter {
        unsafe {
            let mut adapter_desc = mem::zeroed();
            let result = dxgi_adapter.GetDesc(&mut adapter_desc);
            assert!(result.is_ok());

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
            PostMessageA(self.window, WM_CLOSE, WPARAM(0), LPARAM(0));
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
                ReleaseDC(window, self.dc);
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

    // From https://github.com/microsoft/windows-rs/blob/02c4f29d19fbe6d59b2ae0b42262e68d00438f0f/crates/samples/windows/direct2d/src/main.rs#L439
    #[inline]
    pub(crate) fn get_dc(&self) -> DCGuard {
        unsafe { DCGuard::new(GetDC(self.window), Some(self.window)) }
    }
    extern "system" fn wndproc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcA(window, message, wparam, lparam) }
    }
    // The thread that creates the window for off-screen contexts.
    fn thread(sender: Sender<SendableHWND>) {
        unsafe {
            let instance = HINSTANCE::from(GetModuleHandleA(PCSTR::null()).unwrap());
            let window_class_name = PCSTR(&b"SurfmanHiddenWindow\0"[0] as *const u8);
            let mut window_class = mem::zeroed();
            if GetClassInfoA(instance, window_class_name, &mut window_class).is_err() {
                window_class = WNDCLASSA {
                    style: CS_OWNDC,
                    lpfnWndProc: Some(Self::wndproc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: instance,
                    hIcon: HICON::default(),
                    hCursor: HCURSOR::default(),
                    hbrBackground: HBRUSH(COLOR_BACKGROUND.0 as isize),
                    lpszMenuName: PCSTR::null(),
                    lpszClassName: window_class_name,
                };
                let window_class_atom = RegisterClassA(&window_class);
                assert_ne!(window_class_atom, 0);
            }

            let window = CreateWindowExA(
                WINDOW_EX_STYLE(0),
                window_class_name,
                window_class_name,
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                HIDDEN_WINDOW_SIZE,
                HIDDEN_WINDOW_SIZE,
                HWND::default(),
                HMENU::default(),
                instance,
                None,
            );

            sender.send(SendableHWND(window)).unwrap();

            let mut msg: MSG = mem::zeroed();
            while GetMessageA(&mut msg, window, 0, 0) != false {
                TranslateMessage(&msg);
                DispatchMessageA(&msg);
                if msg.message == WM_CLOSE {
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
