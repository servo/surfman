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
use winapi::shared::dxgi::{IDXGIAdapter, IDXGIDevice};
use winapi::shared::minwindef::{self, FALSE, UINT};
use winapi::shared::ntdef::{HANDLE, LPCSTR};
use winapi::shared::windef::{HBRUSH, HDC, HWND};
use winapi::shared::winerror::{self, S_OK};
use winapi::um::d3d11::{D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_SDK_VERSION};
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_HARDWARE;
use winapi::um::libloaderapi;
use winapi::um::winuser::{self, COLOR_BACKGROUND, CS_OWNDC, MSG, WM_CLOSE};
use winapi::um::winuser::{WNDCLASSA, WS_OVERLAPPEDWINDOW};
use wio::com::ComPtr;

pub(crate) const HIDDEN_WINDOW_SIZE: c_int = 16;

const INTEL_PCI_ID: UINT = 0x8086;

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
            let nvidia_gpu_select_variable: *mut i32 = libloaderapi::GetProcAddress(
                current_module,
                NVIDIA_GPU_SELECT_SYMBOL.as_ptr() as LPCSTR,
            ) as *mut i32;
            let amd_gpu_select_variable: *mut i32 = libloaderapi::GetProcAddress(
                current_module,
                AMD_GPU_SELECT_SYMBOL.as_ptr() as LPCSTR,
            ) as *mut i32;
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
    pub(crate) fn get_dc(&self) -> DCGuard {
        unsafe { DCGuard::new(winuser::GetDC(self.window), Some(self.window)) }
    }

    // The thread that creates the window for off-screen contexts.
    fn thread(sender: Sender<SendableHWND>) {
        unsafe {
            let instance = libloaderapi::GetModuleHandleA(ptr::null_mut());
            let window_class_name = &b"SurfmanHiddenWindow\0"[0] as *const u8 as LPCSTR;
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
