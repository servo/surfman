// surfman/surfman/src/platform/windows/angle/surface.rs
//
//! Surface management for Direct3D 11 on Windows using the ANGLE library as a frontend.

use crate::context::ContextID;
use crate::egl::types::EGLSurface;
use crate::egl::{self, EGLint};
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::gl;
use crate::platform::generic::egl::device::EGL_FUNCTIONS;
use crate::platform::generic::egl::error::ToWindowingApiError;
use crate::platform::generic::egl::ffi::EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE;
use crate::platform::generic::egl::ffi::EGL_D3D_TEXTURE_ANGLE;
use crate::platform::generic::egl::ffi::EGL_DXGI_KEYED_MUTEX_ANGLE;
use crate::platform::generic::egl::ffi::EGL_EXTENSION_FUNCTIONS;
use crate::{Error, SurfaceAccess, SurfaceID, SurfaceInfo, SurfaceType};
use super::context::{Context, ContextDescriptor, GL_FUNCTIONS};
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
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser;
use wio::com::ComPtr;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_2D;

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
    pub(crate) egl_surface: EGLSurface,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) context_descriptor: ContextDescriptor,
    pub(crate) win32_objects: Win32Objects,
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

impl Debug for SurfaceTexture {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "SurfaceTexture({:?})", self.surface)
    }
}

pub(crate) enum Win32Objects {
    Window,
    Pbuffer {
        share_handle: HANDLE,
        keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
    }
}

/// Wraps a Windows `HWND` window handle.
#[cfg(not(target_vendor = "uwp"))]
pub struct NativeWidget {
    /// A window handle.
    ///
    /// This can be a top-level window or a control.
    pub window_handle: HWND,
}

/// A placeholder native widget type for UWP, which isn't supported at the moment.
#[cfg(target_vendor = "uwp")]
pub struct NativeWidget;

impl Device {
    /// Creates either a generic or a widget surface, depending on the supplied surface type.
    /// 
    /// Only the given context may ever render to the surface, but generic surfaces can be wrapped
    /// up in a `SurfaceTexture` for reading by other contexts.
    pub fn create_surface(&mut self,
                          context: &Context,
                          _: SurfaceAccess,
                          surface_type: SurfaceType<NativeWidget>)
                          -> Result<Surface, Error> {
        match surface_type {
            SurfaceType::Generic { ref size } => self.create_pbuffer_surface(context, size, None),
            #[cfg(not(target_vendor = "uwp"))]
            SurfaceType::Widget { ref native_widget } => {
                self.create_window_surface(context, native_widget)
            }
            #[cfg(target_vendor = "uwp")]
            SurfaceType::Widget { .. } => Err(Error::UnsupportedOnThisPlatform)
        }
    }

    #[allow(non_snake_case)]
    fn create_pbuffer_surface(&mut self,
                              context: &Context,
                              size: &Size2D<i32>,
                              share_handle: Option<HANDLE>)
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
                let egl_surface = if let Some(share_handle) = share_handle {
                    let surface =
                        egl.CreatePbufferFromClientBuffer(self.egl_display,
                                                          EGL_D3D_TEXTURE_ANGLE,
                                                          share_handle as *const _,
                                                          egl_config,
                                                          attributes.as_ptr());
                    assert_ne!(surface, egl::NO_SURFACE);
                    surface
                } else if share_handle.is_some() {
                    egl::NO_SURFACE
                } else {
                    let surface = egl.CreatePbufferSurface(self.egl_display,
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
                        eglQuerySurfacePointerANGLE(self.egl_display,
                                                    egl_surface,
                                                    EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE as EGLint,
                                                    &mut share_handle);
                    assert_ne!(result, egl::FALSE);
                    assert_ne!(share_handle, INVALID_HANDLE_VALUE);
                    share_handle
                };

                // `mozangle` builds ANGLE with keyed mutexes for sharing. Use the
                // `EGL_ANGLE_keyed_mutex` extension to fetch the keyed mutex so we can grab it.
                let mut keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
                let result = eglQuerySurfacePointerANGLE(
                    self.egl_display,
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

    #[cfg(not(target_vendor = "uwp"))]
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
                let egl_surface = egl.CreateWindowSurface(self.egl_display,
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

    #[cfg(target_vendor = "uwp")]
    fn create_window_surface(&mut self, context: &Context, native_widget: &NativeWidget)
                              -> Result<Surface, Error> {
        Err(Error::UnsupportedOnThisPlatform)
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
    #[allow(non_snake_case)]
    pub fn create_surface_texture(&self, context: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, (Error, Surface)> {
        let share_handle = match surface.win32_objects {
            Win32Objects::Window => return Err((Error::WidgetAttached, surface)),
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

                let local_egl_surface =
                    egl.CreatePbufferFromClientBuffer(self.egl_display,
                                                      EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                                                      share_handle,
                                                      local_egl_config,
                                                      pbuffer_attributes.as_ptr());
                if local_egl_surface == egl::NO_SURFACE {
                    let windowing_api_error = egl.GetError().to_windowing_api_error();
                    return Err((Error::SurfaceImportFailed(windowing_api_error), surface));
                }

                let mut local_keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
                let eglQuerySurfacePointerANGLE = EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE
                                                                         .unwrap();
                let result = eglQuerySurfacePointerANGLE(
                    self.egl_display,
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

                let _guard = self.temporarily_make_context_current(context);

                GL_FUNCTIONS.with(|gl| {
                    // Then bind that surface to the texture.
                    let mut texture = 0;
                    gl.GenTextures(1, &mut texture);
                    debug_assert_ne!(texture, 0);

                    gl.BindTexture(gl::TEXTURE_2D, texture);
                    if egl.BindTexImage(self.egl_display,
                                        local_egl_surface,
                                        egl::BACK_BUFFER as GLint) == egl::FALSE {
                        let windowing_api_error = egl.GetError().to_windowing_api_error();
                        return Err((Error::SurfaceTextureCreationFailed(windowing_api_error),
                                    surface));
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

    /// Destroys a surface.
    /// 
    /// The supplied context must be the context the surface is associated with, or this returns
    /// an `IncompatibleSurface` error.
    /// 
    /// You must explicitly call this method to dispose of a surface. Otherwise, a panic occurs in
    /// the `drop` method.
    pub fn destroy_surface(&self, context: &mut Context, surface: &mut Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                // If the surface is currently bound, unbind it.
                if egl.GetCurrentSurface(egl::READ as EGLint) == surface.egl_surface ||
                        egl.GetCurrentSurface(egl::DRAW as EGLint) == surface.egl_surface {
                    self.make_no_context_current()?;
                }

                egl.DestroySurface(self.egl_display, surface.egl_surface);
                surface.egl_surface = egl::NO_SURFACE;
            }
            Ok(())
        })
    }

    /// Destroys a surface texture and returns the underlying surface.
    /// 
    /// The supplied context must be the same context the surface texture was created with, or an
    /// `IncompatibleSurfaceTexture` error is returned.
    /// 
    /// All surface textures must be explicitly destroyed with this function, or a panic will
    /// occur.
    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, (Error, SurfaceTexture)> {
        unsafe {
            GL_FUNCTIONS.with(|gl| gl.DeleteTextures(1, &surface_texture.gl_texture));
            surface_texture.gl_texture = 0;

            if let Some(ref local_keyed_mutex) = surface_texture.local_keyed_mutex {
                let result = local_keyed_mutex.ReleaseSync(0);
                assert_eq!(result, S_OK);
            }

            EGL_FUNCTIONS.with(|egl| {
                egl.DestroySurface(self.egl_display,
                                   surface_texture.local_egl_surface);
            })
        }

        Ok(surface_texture.surface)
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    /// 
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(&self, _surface: &'s mut Surface)
                                 -> Result<SurfaceDataGuard<'s>, Error> {
        Err(Error::Unimplemented)
    }

    /// Displays the contents of a widget surface on screen.
    /// 
    /// Widget surfaces are internally double-buffered, so changes to them don't show up in their
    /// associated widgets until this method is called.
    /// 
    /// The supplied context must match the context the surface was created with, or an
    /// `IncompatibleSurface` error is returned.
    pub fn present_surface(&self, _: &Context, surface: &mut Surface) -> Result<(), Error> {
        match surface.win32_objects {
            Win32Objects::Window { .. } => {}
            _ => return Err(Error::NoWidgetAttached),
        }

        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                let ok = egl.SwapBuffers(self.egl_display, surface.egl_surface);
                assert_ne!(ok, egl::FALSE);
                Ok(())
            }
        })
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
            framebuffer_object: 0,
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
    #[inline]
    fn id(&self) -> SurfaceID {
        SurfaceID(self.egl_surface as usize)
    }

    #[inline]
    pub(crate) fn uses_keyed_mutex(&self) -> bool {
        match self.win32_objects {
            Win32Objects::Pbuffer { keyed_mutex: Some(_), .. } => true,
            Win32Objects::Pbuffer { keyed_mutex: None, .. } | Win32Objects::Window => false,
        }
    }
}

/// Represents the CPU view of the pixel data of this surface.
pub struct SurfaceDataGuard<'a> {
    phantom: PhantomData<&'a ()>,
}
