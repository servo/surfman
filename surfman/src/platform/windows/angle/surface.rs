//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use crate::context::ContextID;
use crate::egl::types::{EGLSurface, EGLenum};
use crate::egl::{self, EGLint};
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::platform::generic::egl::ffi::EGL_D3D_TEXTURE_ANGLE;
use crate::platform::generic::egl::ffi::EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE;
use crate::platform::generic::egl::ffi::EGL_DXGI_KEYED_MUTEX_ANGLE;
use crate::platform::generic::egl::ffi::EGL_EXTENSION_FUNCTIONS;
use crate::{ContextAttributeFlags, Error, SurfaceAccess, SurfaceID, SurfaceType};
use super::context::{self, Context, ContextDescriptor, GL_FUNCTIONS};
use super::device::Device;

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use std::thread;
use winapi::shared::dxgi::IDXGIKeyedMutex;
use winapi::shared::windef::{HWND, RECT};
use winapi::shared::winerror::S_OK;
use winapi::um::d3d11;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser;
use wio::com::ComPtr;

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::windows::WindowExt;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

pub struct Surface {
    pub(crate) egl_surface: EGLSurface,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) context_descriptor: ContextDescriptor,
    pub(crate) win32_objects: Win32Objects,
}

#[derive(Debug)]
pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) local_egl_surface: EGLSurface,
    pub(crate) local_keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
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
        if self.egl_surface != egl::NO_SURFACE && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

pub(crate) enum Win32Objects {
    Window,
    Pbuffer {
        share_handle: HandleOrTexture,
        keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
    }
}

#[derive(Copy, Clone)]
pub(crate) enum HandleOrTexture {
    Handle(HANDLE),
    Texture(*mut d3d11::ID3D11Texture2D)
}

pub struct NativeWidget {
    pub(crate) window_handle: HWND,
}

impl Device {
    pub fn create_surface(&mut self,
                          context: &Context,
                          _: SurfaceAccess,
                          surface_type: &SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        match *surface_type {
            SurfaceType::Generic { ref size } => self.create_pbuffer_surface(context, size, None),
            SurfaceType::Widget { ref native_widget } => {
                self.create_window_surface(context, native_widget)
            }
        }
    }

    fn create_pbuffer_surface(&mut self,
                              context: &Context,
                              size: &Size2D<i32>,
                              share_handle: Option<HandleOrTexture>)
                              -> Result<Surface, Error> {
        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        unsafe {
            let attributes = [
                egl::WIDTH as EGLint,           size.width as EGLint,
                egl::HEIGHT as EGLint,          size.height as EGLint,
                egl::TEXTURE_FORMAT as EGLint,  egl::TEXTURE_RGBA as EGLint,
                egl::TEXTURE_TARGET as EGLint,  egl::TEXTURE_2D as EGLint,
                egl::NONE as EGLint,            0,
                0,                              0,
            ];

            EGL_FUNCTIONS.with(|egl| {
                let egl_surface = if share_handle.is_some() {
                    egl::NO_SURFACE
                } else {
                    let surface = egl.CreatePbufferSurface(self.native_display.egl_display(),
                                                           egl_config,
                                                           attributes.as_ptr());
                    assert_ne!(surface, egl::NO_SURFACE);
                    surface
                };

                let eglQuerySurfacePointerANGLE =
                    EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE
                                        .expect("Where's the `EGL_ANGLE_query_surface_pointer` \
                                                    extension?");

                let share_handle = if let Some(share_handle) = share_handle {
                    share_handle
                } else {
                    let mut share_handle = INVALID_HANDLE_VALUE;
                    let result =
                        eglQuerySurfacePointerANGLE(self.native_display.egl_display(),
                                                    egl_surface,
                                                    EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE as EGLint,
                                                    &mut share_handle);
                    assert_ne!(result, egl::FALSE);
                    assert_ne!(share_handle, INVALID_HANDLE_VALUE);
                    HandleOrTexture::Handle(share_handle)
                };

                // `mozangle` builds ANGLE with keyed mutexes for sharing. Use the
                // `EGL_ANGLE_keyed_mutex` extension to fetch the keyed mutex so we can grab it.
                let mut keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
                let result = eglQuerySurfacePointerANGLE(
                    self.native_display.egl_display(),
                    egl_surface,
                    EGL_DXGI_KEYED_MUTEX_ANGLE as EGLint,
                    &mut keyed_mutex as *mut *mut IDXGIKeyedMutex as *mut *mut c_void);
                let keyed_mutex = if result != egl::FALSE && !keyed_mutex.is_null() {
                    let keyed_mutex = ComPtr::from_raw(keyed_mutex);
                    keyed_mutex.AddRef();
                    Some(keyed_mutex)
                } else {
                    None
                };

                Ok(Surface {
                    egl_surface,
                    size: *size,
                    context_id: context.id,
                    context_descriptor,
                    win32_objects: Win32Objects::Pbuffer {
                        share_handle,
                        keyed_mutex
                    },
                })
            })
        }
    }

    fn create_window_surface(&mut self, context: &Context, native_widget: &NativeWidget)
                              -> Result<Surface, Error> {
        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        unsafe {
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            let ok = winuser::GetWindowRect(native_widget.window_handle, &mut rect);
            assert_ne!(ok, 0);

            EGL_FUNCTIONS.with(|egl| {
                let attributes = [egl::NONE as EGLint];
                let egl_surface = egl.CreateWindowSurface(self.native_display.egl_display(),
                                                          egl_config,
                                                          native_widget.window_handle as _,
                                                          attributes.as_ptr());
                assert_ne!(egl_surface, egl::NO_SURFACE);

                Ok(Surface {
                    egl_surface,
                    size: Size2D::new(rect.right - rect.left, rect.bottom - rect.top),
                    context_id: context.id,
                    context_descriptor,
                    win32_objects: Win32Objects::Window,
                })
            })
        }
    }

    pub unsafe fn create_surface_from_texture(
        &mut self,
        context: &Context,
        size: &Size2D<i32>,
        texture: *mut d3d11::ID3D11Texture2D
    ) -> Result<Surface, Error> {
        self.create_pbuffer_surface(context, size, Some(HandleOrTexture::Texture(texture)))
    }

    pub fn create_surface_texture(&self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        let share_handle = match surface.win32_objects {
            Win32Objects::Window => return Err(Error::WidgetAttached),
            Win32Objects::Pbuffer { share_handle, .. } => share_handle,
        };

        let local_egl_config = self.context_descriptor_to_egl_config(&surface.context_descriptor);
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                // First, create an EGL surface local to this thread.
                let pbuffer_attributes = [
                    egl::WIDTH as EGLint,           surface.size.width,
                    egl::HEIGHT as EGLint,          surface.size.height,
                    egl::TEXTURE_FORMAT as EGLint,  egl::TEXTURE_RGBA as EGLint,
                    egl::TEXTURE_TARGET as EGLint,  egl::TEXTURE_2D as EGLint,
                    egl::NONE as EGLint,            0,
                    0,                              0,
                ];

                // FIXME(pcwalton): I'm fairly sure this is undefined behavior! We need to figure
                // out how to use share handles for jdm's use case.
                let (buffer_type, client_buffer) = match share_handle {
                    HandleOrTexture::Handle(handle) => {
                        (EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE, handle)
                    }
                    HandleOrTexture::Texture(texture) => {
                        (EGL_D3D_TEXTURE_ANGLE, texture as _)
                    }
                };

                let local_egl_surface =
                    egl.CreatePbufferFromClientBuffer(self.native_display.egl_display(),
                                                      buffer_type,
                                                      client_buffer,
                                                      local_egl_config,
                                                      pbuffer_attributes.as_ptr());
                if local_egl_surface == egl::NO_SURFACE {
                    let windowing_api_error = egl.GetError().to_windowing_api_error();
                    return Err(Error::SurfaceImportFailed(windowing_api_error));
                }

                let mut local_keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
                let eglQuerySurfacePointerANGLE = EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE
                                                                         .unwrap();
                let result = eglQuerySurfacePointerANGLE(
                    self.native_display.egl_display(),
                    local_egl_surface,
                    EGL_DXGI_KEYED_MUTEX_ANGLE as EGLint,
                    &mut local_keyed_mutex as *mut *mut IDXGIKeyedMutex as *mut *mut c_void);
                let local_keyed_mutex = if result != egl::FALSE && !local_keyed_mutex.is_null() {
                    let local_keyed_mutex = ComPtr::from_raw(local_keyed_mutex);
                    local_keyed_mutex.AddRef();

                    let result = local_keyed_mutex.AcquireSync(0, INFINITE);
                    assert_eq!(result, S_OK);

                    Some(local_keyed_mutex)
                } else {
                    None
                };

                GL_FUNCTIONS.with(|gl| {
                    // Then bind that surface to the texture.
                    let mut texture = 0;
                    gl.GenTextures(1, &mut texture);
                    debug_assert_ne!(texture, 0);

                    gl.BindTexture(gl::TEXTURE_2D, texture);
                    if egl.BindTexImage(self.native_display.egl_display(),
                                        local_egl_surface,
                                        egl::BACK_BUFFER as GLint) == egl::FALSE {
                        let windowing_api_error = egl.GetError().to_windowing_api_error();
                        return Err(Error::SurfaceTextureCreationFailed(windowing_api_error));
                    }

                    // Initialize the texture, for convenience.
                    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                    gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                    gl.TexParameteri(gl::TEXTURE_2D,
                                     gl::TEXTURE_WRAP_S,
                                     gl::CLAMP_TO_EDGE as GLint);
                    gl.TexParameteri(gl::TEXTURE_2D,
                                     gl::TEXTURE_WRAP_T,
                                     gl::CLAMP_TO_EDGE as GLint);

                    gl.BindTexture(gl::TEXTURE_2D, 0);
                    debug_assert_eq!(gl.GetError(), gl::NO_ERROR);

                    Ok(SurfaceTexture {
                        surface,
                        local_egl_surface,
                        local_keyed_mutex,
                        gl_texture: texture,
                        phantom: PhantomData,
                    })
                })
            }
        })
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // leak!
            surface.egl_surface = egl::NO_SURFACE;
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                // If the surface is currently bound, unbind it.
                if egl.GetCurrentSurface(egl::READ as EGLint) == surface.egl_surface ||
                        egl.GetCurrentSurface(egl::DRAW as EGLint) == surface.egl_surface {
                    self.make_no_context_current()?;
                }

                egl.DestroySurface(self.native_display.egl_display(), surface.egl_surface);
                surface.egl_surface = egl::NO_SURFACE;
            }
            Ok(())
        })
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        unsafe {
            GL_FUNCTIONS.with(|gl| gl.DeleteTextures(1, &surface_texture.gl_texture));
            surface_texture.gl_texture = 0;

            if let Some(ref local_keyed_mutex) = surface_texture.local_keyed_mutex {
                let result = local_keyed_mutex.ReleaseSync(0);
                assert_eq!(result, S_OK);
            }

            EGL_FUNCTIONS.with(|egl| {
                egl.DestroySurface(self.native_display.egl_display(),
                                   surface_texture.local_egl_surface);
            })
        }

        Ok(surface_texture.surface)
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    #[inline]
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        self.present_surface_without_context(surface)
    }

    #[inline]
    pub fn lock_surface_data<'s>(&self, _surface: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
        Err(Error::Unimplemented)
    }

    pub(crate) fn present_surface_without_context(&self, surface: &mut Surface)
                                                  -> Result<(), Error> {
        match surface.win32_objects {
            Win32Objects::Window { .. } => {}
            _ => return Err(Error::NoWidgetAttached),
        }

        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                let ok = egl.SwapBuffers(self.native_display.egl_display(), surface.egl_surface);
                assert_ne!(ok, egl::FALSE);
                Ok(())
            }
        })
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        SurfaceID(self.egl_surface as usize)
    }

    #[inline]
    pub fn context_id(&self) -> ContextID {
        self.context_id
    }

    #[inline]
    pub(crate) fn uses_keyed_mutex(&self) -> bool {
        match self.win32_objects {
            Win32Objects::Pbuffer { keyed_mutex: Some(_), .. } => true,
            Win32Objects::Pbuffer { keyed_mutex: None, .. } | Win32Objects::Window => false,
        }
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.gl_texture
    }
}

impl NativeWidget {
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn from_winit_window(window: &Window) -> NativeWidget {
        NativeWidget { window_handle: window.get_hwnd() as HWND }
    }
}

pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
