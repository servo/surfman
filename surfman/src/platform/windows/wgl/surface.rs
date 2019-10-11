// surfman/src/platform/windows/wgl/surface.rs
//
//! An implementation of the GPU device for Windows using WGL/Direct3D interoperability.

use crate::ContextID;
use crate::renderbuffers::Renderbuffers;
use super::context::WGL_EXTENSION_FUNCTIONS;

use euclid::default::Size2D;
use gl::types::GLuint;
use std::ptr;
use wio::com::ComPtr;

pub struct Surface {
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) win32_objects: Win32Objects,
    pub(crate) destroyed: bool,
}

pub(crate) enum Win32Objects {
    Texture {
        d3d11_texture: ComPtr<ID3D11Texture>,
        dxgi_share_handle: HANDLE,
        gl_dx_interop_object: HANDLE,
        gl_texture: GLuint,
        gl_framebuffer: GLuint,
        renderbuffers: Renderbuffers,
    },
    Widget,
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
            SurfaceType::Widget { native_widget } => {
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
            if !SUCCEEDED(result) {
                return Err(Error::SurfaceCreationFailed(WindowingApiError::Failed));
            }
            assert!(!d3d11_texture.is_null());
            let d3d11_texture = ComPtr::from_raw(d3d11_texture);

            // Upcast it to a DXGI resource.
            let mut dxgi_resource = ptr::null_mut();
            result = d3d11_texture.QueryInterface(&IDXGIResource::uuidof(), &mut dxgi_resource);
            assert!(SUCCEEDED(result));

            // Get the share handle. We'll need it both to bind to GL and to share the texture
            // across contexts.
            let mut dxgi_share_handle = INVALID_HANDLE_VALUE;
            result = dxgi_resource.GetSharedHandle(&mut dxgi_share_handle);
            assert!(SUCCEEDED(result));
            assert_ne!(dxgi_share_handle, INVALID_HANDLE_VALUE);

            // Tell GL about the share handle.
            let ok = (dx_interop_functions.DXSetResourceShareHandleNV)(d3d11_texture.as_raw(),
                                                                       dxgi_share_handle);
            assert_ne!(ok, FALSE);

            GL_FUNCTIONS.with(|gl| {
                // Make our texture object on the GL side.
                let mut gl_texture = 0;
                gl.GenTextures(1, &mut gl_texture);

                // Bind the GL texture to the D3D11 texture.
                let gl_dx_interop_object =
                    (dx_interop_functions.DXRegisterObjectNV)(self.gl_dx_interop_device,
                                                              d3d11_texture,
                                                              gl_texture,
                                                              gl::TEXTURE_2D,
                                                              WGL_ACCESS_READ_WRITE_NV);
                assert_ne!(gl_dx_interop_object, INVALID_HANDLE_VALUE);

                // Build our FBO.
                let mut gl_framebuffer = 0;
                gl.GenFramebuffers(1, &mut gl_framebuffer);
                let _guard = self.temporarily_bind_framebuffer(gl_framebuffer);

                // Attach the reflected D3D11 texture to that FBO.
                gl.FramebufferTexture2D(gl::FRAMEBUFFER,
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
                        dxgi_share_handle,
                        gl_dx_interop_object,
                        gl_texture,
                        gl_framebuffer,
                        renderbuffers,
                    },
                    destroyed: false,
                })
            })
        }
    }
}
