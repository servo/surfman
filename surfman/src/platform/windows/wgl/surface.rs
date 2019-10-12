// surfman/src/platform/windows/wgl/surface.rs
//
//! An implementation of the GPU device for Windows using WGL/Direct3D interoperability.

use crate::error::WindowingApiError;
use crate::renderbuffers::Renderbuffers;
use crate::{ContextID, Error, SurfaceID};
use super::context::{Context, WGL_EXTENSION_FUNCTIONS};
use super::device::Device;

use crate::gl::types::{GLenum, GLuint};
use crate::gl::{self, Gl};
use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::thread;
use winapi::Interface;
use winapi::shared::dxgi::IDXGIResource;
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::minwindef::{FALSE, UINT};
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::HWND;
use winapi::shared::winerror;
use winapi::um::d3d11::{D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE};
use winapi::um::d3d11::{D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX, D3D11_TEXTURE2D_DESC};
use winapi::um::d3d11::{D3D11_USAGE_DEFAULT, ID3D11Texture2D};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winuser;
use wio::com::ComPtr;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

const WGL_ACCESS_READ_WRITE_NV: GLenum = 0x0001;

pub struct Surface {
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) win32_objects: Win32Objects,
    pub(crate) destroyed: bool,
}

pub(crate) enum Win32Objects {
    Texture {
        d3d11_texture: ComPtr<ID3D11Texture2D>,
        dxgi_share_handle: HANDLE,
        gl_dx_interop_object: HANDLE,
        gl_texture: GLuint,
        gl_framebuffer: GLuint,
        renderbuffers: Renderbuffers,
    },
    Widget {
        window_handle: HWND,
    },
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.destroyed && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

pub enum SurfaceType {
    Generic { size: Size2D<i32> },
    Widget { native_widget: NativeWidget },
}

pub struct NativeWidget {
    pub(crate) window_handle: HWND,
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, surface_type: &SurfaceType)
                          -> Result<Surface, Error> {
        match *surface_type {
            SurfaceType::Generic { ref size } => self.create_generic_surface(context, size),
            SurfaceType::Widget { ref native_widget } => {
                self.create_widget_surface(context, native_widget)
            }
        }
    }

    fn create_generic_surface(&mut self, context: &Context, size: &Size2D<i32>)
                              -> Result<Surface, Error> {
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
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET,
                CPUAccessFlags: 0,
                MiscFlags: D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX,
            };
            let mut d3d11_texture = ptr::null_mut();
            let mut result = self.d3d11_device.CreateTexture2D(&d3d11_texture2d_desc,
                                                               ptr::null(),
                                                               &mut d3d11_texture);
            if !winerror::SUCCEEDED(result) {
                return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
            }
            assert!(!d3d11_texture.is_null());
            let d3d11_texture = ComPtr::from_raw(d3d11_texture);

            // Upcast it to a DXGI resource.
            let mut dxgi_resource: *mut IDXGIResource = ptr::null_mut();
            result = d3d11_texture.QueryInterface(
                &IDXGIResource::uuidof(),
                &mut dxgi_resource as *mut *mut IDXGIResource as *mut *mut c_void);
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
                dxgi_share_handle);
            assert_ne!(ok, FALSE);

            // Make our texture object on the GL side.
            let mut gl_texture = 0;
            context.gl.GenTextures(1, &mut gl_texture);

            // Bind the GL texture to the D3D11 texture.
            let gl_dx_interop_object =
                (dx_interop_functions.DXRegisterObjectNV)(self.gl_dx_interop_device,
                                                          d3d11_texture.as_raw() as *mut c_void,
                                                          gl_texture,
                                                          gl::TEXTURE_2D,
                                                          WGL_ACCESS_READ_WRITE_NV);
            assert_ne!(gl_dx_interop_object, INVALID_HANDLE_VALUE);

            // Build our FBO.
            let mut gl_framebuffer = 0;
            context.gl.GenFramebuffers(1, &mut gl_framebuffer);
            let _guard = self.temporarily_bind_framebuffer(gl_framebuffer);

            // Attach the reflected D3D11 texture to that FBO.
            context.gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                            gl::COLOR_ATTACHMENT0,
                                            SURFACE_GL_TEXTURE_TARGET,
                                            gl_texture,
                                            0);

            // Create renderbuffers as appropriate, and attach them.
            let context_descriptor = self.context_descriptor(context);
            let context_attributes = self.context_descriptor_attributes(&context_descriptor);
            let renderbuffers = Renderbuffers::new(&size, &context_attributes);
            renderbuffers.bind_to_current_framebuffer();

            Ok(Surface {
                size: *size,
                context_id: context.id,
                win32_objects: Win32Objects::Texture {
                    d3d11_texture,
                    dxgi_share_handle,
                    gl_dx_interop_object,
                    gl_texture,
                    gl_framebuffer,
                    renderbuffers,
                },
                destroyed: false,
            })
        }
    }

    fn create_widget_surface(&mut self, context: &Context, native_widget: &NativeWidget)
                              -> Result<Surface, Error> {
        unsafe {
            let mut widget_rect = mem::zeroed();
            let ok = winuser::GetWindowRect(native_widget.window_handle, &mut widget_rect);
            if ok == FALSE {
                return Err(Error::InvalidNativeWidget);
            }

            Ok(Surface {
                size: Size2D::new(widget_rect.right - widget_rect.left,
                                  widget_rect.bottom - widget_rect.top),
                context_id: context.id,
                win32_objects: Win32Objects::Widget {
                    window_handle: native_widget.window_handle,
                },
                destroyed: false,
            })
        }
    }
}

impl Surface {
    pub fn id(&self) -> SurfaceID {
        match self.win32_objects {
            Win32Objects::Texture { ref d3d11_texture, .. } => {
                SurfaceID((*d3d11_texture).as_raw() as usize)
            }
            Win32Objects::Widget { window_handle } => SurfaceID(window_handle as usize),
        }
    }
}

