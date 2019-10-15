// surfman/src/platform/windows/wgl/device.rs
//
//! An implementation of the GPU device for Windows using the WGL API.

use crate::{Error, GLApi};
use super::adapter::Adapter;
use super::context::{WGLExtensionFunctions, WGL_EXTENSION_FUNCTIONS};

use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use winapi::shared::minwindef::{self, FALSE, LPVOID, UINT};
use winapi::shared::ntdef::{HANDLE, LPCSTR};
use winapi::shared::windef::{HBRUSH, HDC, HWND};
use winapi::shared::winerror;
use winapi::um::d3d11::{D3D11CreateDevice, D3D11_SDK_VERSION, ID3D11Device, ID3D11DeviceContext};
use winapi::um::d3dcommon::D3D_DRIVER_TYPE_HARDWARE;
use winapi::um::libloaderapi;
use winapi::um::winuser::{self, COLOR_BACKGROUND, CS_OWNDC, MSG, WM_CLOSE, WM_DESTROY};
use winapi::um::winuser::{WNDCLASSA, WS_OVERLAPPEDWINDOW};
use wio::com::ComPtr;

pub(crate) const HIDDEN_WINDOW_SIZE: c_int = 16;

struct SendableHWND(HWND);

unsafe impl Send for SendableHWND {}

#[allow(dead_code)]
pub struct Device {
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d11_device_context: ComPtr<ID3D11DeviceContext>,
    pub(crate) gl_dx_interop_device: HANDLE,
    pub(crate) hidden_window: HiddenWindow,
}

impl Drop for Device {
    fn drop(&mut self) {
        let dx_interop_functions = WGL_EXTENSION_FUNCTIONS.dx_interop_functions.as_ref().unwrap();
        unsafe {
            (dx_interop_functions.DXCloseDeviceNV)(self.gl_dx_interop_device);
        }
    }
}

impl Device {
    pub fn new(_: &Adapter) -> Result<Device, Error> {
        let dx_interop_functions = match WGL_EXTENSION_FUNCTIONS.dx_interop_functions {
            Some(ref dx_interop_functions) => dx_interop_functions,
            None => return Err(Error::RequiredExtensionUnavailable),
        };

        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let mut d3d11_device_context = ptr::null_mut();
            let result = D3D11CreateDevice(ptr::null_mut(),
                                           D3D_DRIVER_TYPE_HARDWARE,
                                           ptr::null_mut(),
                                           0,
                                           ptr::null_mut(),
                                           0,
                                           D3D11_SDK_VERSION,
                                           &mut d3d11_device,
                                           &mut d3d11_feature_level,
                                           &mut d3d11_device_context);
            if !winerror::SUCCEEDED(result) {
                return Err(Error::DeviceOpenFailed);
            }
            let d3d11_device = ComPtr::from_raw(d3d11_device);
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let gl_dx_interop_device =
                (dx_interop_functions.DXOpenDeviceNV)(d3d11_device.as_raw() as *mut c_void);
            assert!(!gl_dx_interop_device.is_null());

            let hidden_window = HiddenWindow::new();
            Ok(Device { d3d11_device, d3d11_device_context, gl_dx_interop_device, hidden_window })
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }

    #[inline]
    pub fn gl_api() -> GLApi {
        GLApi::GL
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
            winuser::SendMessageA(self.window, WM_CLOSE, 0, 0);
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
        HiddenWindow { window, join_handle: Some(join_handle) }
    }

    #[inline]
    pub(crate) fn get_dc(&self) -> DCGuard {
        unsafe {
            DCGuard::new(winuser::GetDC(self.window), Some(self.window))
        }
    }

    // The thread that creates the window for off-screen contexts.
    fn thread(sender: Sender<SendableHWND>) {
        unsafe {
            let instance = libloaderapi::GetModuleHandleA(ptr::null_mut());
            let window_class_name = &b"SurfmanHiddenWindow"[0] as *const u8 as LPCSTR;
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

            let window = winuser::CreateWindowExA(0,
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
                                                  ptr::null_mut());

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
        DCGuard { dc, window, phantom: PhantomData }
    }
}
