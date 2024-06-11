// surfman/src/platform/windows/wgl/surface.rs
//
//! An implementation of the GPU device for Windows using WGL/Direct3D interoperability.

use super::context::{self, Context, WGL_EXTENSION_FUNCTIONS};
use super::device::Device;
use crate::error::WindowingApiError;
use crate::renderbuffers::Renderbuffers;
use crate::{ContextID, Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType};

use crate::gl;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl_utils;
use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::thread;
use winapi::shared::dxgi::IDXGIResource;
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::minwindef::{FALSE, UINT};
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::HWND;
use winapi::shared::winerror;
use winapi::um::d3d11::{ID3D11Texture2D, D3D11_USAGE_DEFAULT};
use winapi::um::d3d11::{D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE};
use winapi::um::d3d11::{D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX, D3D11_TEXTURE2D_DESC};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::wingdi;
use winapi::um::winuser;
use winapi::Interface;
use wio::com::ComPtr;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

const WGL_ACCESS_READ_ONLY_NV: GLenum = 0x0000;
const WGL_ACCESS_READ_WRITE_NV: GLenum = 0x0001;

/// Represents a hardware buffer of pixels that can be rendered to via the CPU or GPU and either
/// displayed in a native widget or bound to a texture for reading.
///
/// Surfaces come in two varieties: generic and widget surfaces. Generic surfaces can be bound to a
/// texture but cannot be displayed in a widget (without using other APIs such as Core Animation,
/// DirectComposition, or XPRESENT). Widget surfaces are the opposite: they can be displayed in a
/// widget but not bound to a texture.
///
/// Surfaces are specific to a given context and cannot be rendered to from any context other than
/// the one they were created with. However, they can be *read* from any context on any thread (as
/// long as that context shares the same adapter and connection), by wrapping them in a
/// `SurfaceTexture`.
///
/// Depending on the platform, each surface may be internally double-buffered.
///
/// Surfaces must be destroyed with the `destroy_surface()` method, or a panic will occur.
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

/// Represents an OpenGL texture that wraps a surface.
///
/// Reading from the associated OpenGL texture reads from the surface. It is undefined behavior to
/// write to such a texture (e.g. by binding it to a framebuffer and rendering to that
/// framebuffer).
///
/// Surface textures are local to a context, but that context does not have to be the same context
/// as that associated with the underlying surface. The texture must be destroyed with the
/// `destroy_surface_texture()` method, or a panic will occur.
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    #[allow(dead_code)]
    pub(crate) local_d3d11_texture: ComPtr<ID3D11Texture2D>,
    local_gl_dx_interop_object: HANDLE,
    pub(crate) gl_texture: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
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

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

/// Wraps a Windows `HWND` window handle.
pub struct NativeWidget {
    /// A window handle.
    ///
    /// This can be a top-level window or a control.
    pub window_handle: HWND,
}

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    ///
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(
        &mut self,
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
        &mut self,
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
            let mut gl_texture = 0;
            context.gl.GenTextures(1, &mut gl_texture);

            // Bind the GL texture to the D3D11 texture.
            let gl_dx_interop_object = (dx_interop_functions.DXRegisterObjectNV)(
                self.gl_dx_interop_device,
                d3d11_texture.as_raw() as *mut c_void,
                gl_texture,
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
            let mut gl_framebuffer = 0;
            context.gl.GenFramebuffers(1, &mut gl_framebuffer);
            let _guard = self.temporarily_bind_framebuffer(context, gl_framebuffer);

            // Attach the reflected D3D11 texture to that FBO.
            context.gl.FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                SURFACE_GL_TEXTURE_TARGET,
                gl_texture,
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
                    gl_texture,
                    gl_framebuffer,
                    renderbuffers,
                },
                destroyed: false,
            })
        }
    }

    fn create_widget_surface(
        &mut self,
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
                context::set_dc_pixel_format(window_dc, pixel_format);
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

                    gl_utils::destroy_framebuffer(&context.gl, *gl_framebuffer);
                    *gl_framebuffer = 0;

                    context.gl.DeleteTextures(1, gl_texture);
                    *gl_texture = 0;

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
            let mut gl_texture = 0;
            context.gl.GenTextures(1, &mut gl_texture);

            // Register that texture with GL/DX interop.
            let mut local_gl_dx_interop_object = (dx_interop_functions.DXRegisterObjectNV)(
                self.gl_dx_interop_device,
                local_d3d11_texture.as_raw() as *mut c_void,
                gl_texture,
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
            context.gl.BindTexture(gl::TEXTURE_2D, gl_texture);
            context
                .gl
                .TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            context
                .gl
                .TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            context.gl.TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            context.gl.TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );

            // Finish up.
            Ok(SurfaceTexture {
                surface,
                local_d3d11_texture,
                local_gl_dx_interop_object,
                gl_texture,
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
            context.gl.DeleteTextures(1, &surface_texture.gl_texture);
            surface_texture.gl_texture = 0;
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
        let window_handle = match surface.win32_objects {
            Win32Objects::Widget { window_handle } => window_handle,
            _ => return Err(Error::NoWidgetAttached),
        };

        unsafe {
            let dc = winuser::GetDC(window_handle);
            let ok = wingdi::SwapBuffers(dc);
            assert_ne!(ok, FALSE);
            winuser::ReleaseDC(window_handle, dc);
            Ok(())
        }
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        _scontext: &Context,
        surface: &mut Surface,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        surface.size = size;
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
                Win32Objects::Widget { .. } => 0,
            },
        }
    }

    /// Returns the OpenGL texture object containing the contents of this surface.
    ///
    /// It is only legal to read from, not write to, this texture object.
    #[inline]
    pub fn surface_texture_object(&self, surface_texture: &SurfaceTexture) -> GLuint {
        surface_texture.gl_texture
    }
}

impl Surface {
    pub(crate) fn id(&self) -> SurfaceID {
        match self.win32_objects {
            Win32Objects::Texture {
                ref d3d11_texture, ..
            } => SurfaceID((*d3d11_texture).as_raw() as usize),
            Win32Objects::Widget { window_handle } => SurfaceID(window_handle as usize),
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
