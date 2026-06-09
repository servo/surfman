//! A thread-local handle to the device.

use super::connection::Connection;
use super::context::{Context, ContextDescriptor};
use super::surface::{Surface, Synchronization, Win32Objects};
use crate::angle::context::NativeContext;
use crate::angle::surface::{NativeWidget, SurfaceDataGuard, SurfaceTexture};
use crate::base::egl::context::{self, CurrentContextGuard};
use crate::base::egl::device::EGL_FUNCTIONS;
use crate::base::egl::error::ToWindowingApiError;
use crate::base::egl::ffi::{
    EGL_D3D11_DEVICE_ANGLE, EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE, EGL_D3D_TEXTURE_ANGLE,
    EGL_DEVICE_EXT, EGL_DXGI_KEYED_MUTEX_ANGLE, EGL_EXTENSION_FUNCTIONS, EGL_NO_DEVICE_EXT,
    EGL_PLATFORM_DEVICE_EXT,
};
use crate::base::egl::surface::ExternalEGLSurfaces;
use crate::context::ContextID;
use crate::context::CREATE_CONTEXT_MUTEX;
use crate::egl;
use crate::egl::types::{EGLAttrib, EGLConfig, EGLDeviceEXT, EGLDisplay, EGLSurface, EGLint};
use crate::gl;
use crate::surface::Framebuffer;
use crate::GLApi;
use crate::Gl;
use crate::{ContextAttributes, Error, SurfaceInfo};
use crate::{SurfaceAccess, SurfaceType};
use euclid::default::Size2D;
use glow::HasContext;
use std::cell::{RefCell, RefMut};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use winapi::shared::dxgi::IDXGIKeyedMutex;
use winapi::shared::dxgi::{self, IDXGIAdapter, IDXGIDevice, IDXGIFactory1};
use winapi::shared::minwindef::UINT;
use winapi::shared::winerror::{self, S_OK};
use winapi::um::d3d11;
use winapi::um::d3d11::{D3D11CreateDevice, ID3D11Device, D3D11_SDK_VERSION};
use winapi::um::d3dcommon::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_9_3,
};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winbase::INFINITE;
use winapi::Interface;
use wio::com::ComPtr;

const SURFACE_GL_TEXTURE_TARGET: u32 = gl::TEXTURE_2D;

thread_local! {
    static DXGI_FACTORY: RefCell<Option<ComPtr<IDXGIFactory1>>> = RefCell::new(None);
}

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Clone)]
pub struct Adapter {
    pub(crate) dxgi_adapter: ComPtr<IDXGIAdapter>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
}

unsafe impl Send for Adapter {}

/// A thread-local handle to a device.
///
/// Devices contain most of the relevant surface management methods.
pub struct Device {
    pub(crate) egl_display: EGLDisplay,
    pub(crate) d3d11_device: ComPtr<ID3D11Device>,
    pub(crate) d3d_driver_type: D3D_DRIVER_TYPE,
    pub(crate) display_is_owned: bool,
}

pub(crate) enum VendorPreference {
    None,
    Prefer(UINT),
    Avoid(UINT),
}

/// Wraps a Direct3D 11 device and its associated EGL display.
#[derive(Clone)]
pub struct NativeDevice {
    /// The ANGLE EGL display.
    pub egl_display: EGLDisplay,
    /// The Direct3D 11 device that the ANGLE EGL display was created with.
    pub d3d11_device: *mut ID3D11Device,
    /// The Direct3D driver type that the device was created with.
    pub d3d_driver_type: D3D_DRIVER_TYPE,
}

impl Adapter {
    pub(crate) fn new(
        d3d_driver_type: D3D_DRIVER_TYPE,
        vendor_preference: VendorPreference,
    ) -> Result<Adapter, Error> {
        unsafe {
            let dxgi_factory = DXGI_FACTORY.with(|dxgi_factory_slot| {
                let mut dxgi_factory_slot: RefMut<Option<ComPtr<IDXGIFactory1>>> =
                    dxgi_factory_slot.borrow_mut();
                if dxgi_factory_slot.is_none() {
                    let mut dxgi_factory: *mut IDXGIFactory1 = ptr::null_mut();
                    let result = dxgi::CreateDXGIFactory1(
                        &IDXGIFactory1::uuidof(),
                        &mut dxgi_factory as *mut *mut IDXGIFactory1 as *mut *mut c_void,
                    );
                    if !winerror::SUCCEEDED(result) {
                        return Err(Error::Failed);
                    }
                    assert!(!dxgi_factory.is_null());
                    *dxgi_factory_slot = Some(ComPtr::from_raw(dxgi_factory));
                }
                Ok((*dxgi_factory_slot).clone().unwrap())
            })?;

            // Find the first adapter that matches the vendor preference.
            let mut adapter_index = 0;
            loop {
                let mut dxgi_adapter_1 = ptr::null_mut();
                let result = (*dxgi_factory).EnumAdapters1(adapter_index, &mut dxgi_adapter_1);
                if !winerror::SUCCEEDED(result) {
                    break;
                }
                assert!(!dxgi_adapter_1.is_null());
                let dxgi_adapter_1 = ComPtr::from_raw(dxgi_adapter_1);

                let mut adapter_desc = mem::zeroed();
                let result = (*dxgi_adapter_1).GetDesc1(&mut adapter_desc);
                assert_eq!(result, S_OK);

                let choose_this = match vendor_preference {
                    VendorPreference::Prefer(vendor_id) => vendor_id == adapter_desc.VendorId,
                    VendorPreference::Avoid(vendor_id) => vendor_id != adapter_desc.VendorId,
                    VendorPreference::None => true,
                };
                if choose_this {
                    let mut dxgi_adapter: *mut IDXGIAdapter = ptr::null_mut();
                    let result = (*dxgi_adapter_1).QueryInterface(
                        &IDXGIAdapter::uuidof(),
                        &mut dxgi_adapter as *mut *mut IDXGIAdapter as *mut *mut c_void,
                    );
                    assert_eq!(result, S_OK);
                    let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

                    return Ok(Adapter {
                        dxgi_adapter,
                        d3d_driver_type,
                    });
                }

                adapter_index += 1;
            }

            // Fallback: Go with the first adapter.
            let mut dxgi_adapter_1 = ptr::null_mut();
            let result = (*dxgi_factory).EnumAdapters1(0, &mut dxgi_adapter_1);
            if !winerror::SUCCEEDED(result) {
                return Err(Error::NoAdapterFound);
            }
            assert!(!dxgi_adapter_1.is_null());
            let dxgi_adapter_1 = ComPtr::from_raw(dxgi_adapter_1);

            let mut dxgi_adapter: *mut IDXGIAdapter = ptr::null_mut();
            let result = (*dxgi_adapter_1).QueryInterface(
                &IDXGIAdapter::uuidof(),
                &mut dxgi_adapter as *mut *mut IDXGIAdapter as *mut *mut c_void,
            );
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Ok(Adapter {
                dxgi_adapter,
                d3d_driver_type,
            })
        }
    }

    /// Create an Adapter instance wrapping an existing DXGI adapter.
    pub fn from_dxgi_adapter(adapter: ComPtr<IDXGIAdapter>) -> Adapter {
        Adapter {
            dxgi_adapter: adapter,
            d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
        }
    }
}

impl Device {
    #[allow(non_snake_case)]
    pub(crate) fn new(adapter: &Adapter) -> Result<Device, Error> {
        let d3d_driver_type = adapter.d3d_driver_type;
        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let d3d11_adapter = if d3d_driver_type == D3D_DRIVER_TYPE_WARP {
                ptr::null_mut()
            } else {
                adapter.dxgi_adapter.as_raw()
            };
            let result = D3D11CreateDevice(
                d3d11_adapter,
                d3d_driver_type,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                0,
                D3D11_SDK_VERSION,
                &mut d3d11_device,
                &mut d3d11_feature_level,
                ptr::null_mut(),
            );
            if !winerror::SUCCEEDED(result) {
                return Err(Error::DeviceOpenFailed);
            }
            debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
            let d3d11_device = ComPtr::from_raw(d3d11_device);

            let eglCreateDeviceANGLE = EGL_EXTENSION_FUNCTIONS
                .CreateDeviceANGLE
                .expect("Where's the `EGL_ANGLE_device_creation` extension?");
            let egl_device = eglCreateDeviceANGLE(
                EGL_D3D11_DEVICE_ANGLE as EGLint,
                d3d11_device.as_raw() as *mut c_void,
                ptr::null_mut(),
            );
            assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

            EGL_FUNCTIONS.with(|egl| {
                let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
                let egl_display = egl.GetPlatformDisplay(
                    EGL_PLATFORM_DEVICE_EXT,
                    egl_device as *mut c_void,
                    &attribs[0],
                );
                assert_ne!(egl_display, egl::NO_DISPLAY);

                // I don't think this should ever fail.
                let (mut major_version, mut minor_version) = (0, 0);
                let result = egl.Initialize(egl_display, &mut major_version, &mut minor_version);
                assert_ne!(result, egl::FALSE);

                Ok(Device {
                    egl_display,
                    d3d11_device,
                    d3d_driver_type,
                    display_is_owned: true,
                })
            })
        }
    }

    pub(crate) fn from_native_device(native_device: NativeDevice) -> Result<Device, Error> {
        unsafe {
            (*native_device.d3d11_device).AddRef();
            Ok(Device {
                egl_display: native_device.egl_display,
                d3d11_device: ComPtr::from_raw(native_device.d3d11_device),
                d3d_driver_type: native_device.d3d_driver_type,
                display_is_owned: false,
            })
        }
    }

    #[allow(non_snake_case)]
    pub(crate) fn from_egl_display(egl_display: EGLDisplay) -> Result<Device, Error> {
        let eglQueryDisplayAttribEXT = EGL_EXTENSION_FUNCTIONS
            .QueryDisplayAttribEXT
            .expect("Where's the `EGL_EXT_device_query` extension?");
        let eglQueryDeviceAttribEXT = EGL_EXTENSION_FUNCTIONS
            .QueryDeviceAttribEXT
            .expect("Where's the `EGL_EXT_device_query` extension?");
        let mut angle_device: EGLAttrib = 0;
        let result = eglQueryDisplayAttribEXT(
            egl_display,
            EGL_DEVICE_EXT as EGLint,
            &mut angle_device as *mut EGLAttrib,
        );
        if result == egl::FALSE {
            return Err(Error::DeviceOpenFailed);
        }
        let mut device: EGLAttrib = 0;
        let result = eglQueryDeviceAttribEXT(
            angle_device as EGLDeviceEXT,
            EGL_D3D11_DEVICE_ANGLE as EGLint,
            &mut device as *mut EGLAttrib,
        );
        if result == egl::FALSE {
            return Err(Error::DeviceOpenFailed);
        }
        let d3d11_device = device as *mut ID3D11Device;

        unsafe {
            (*d3d11_device).AddRef();
            Ok(Device {
                egl_display: egl_display,
                d3d11_device: ComPtr::from_raw(d3d11_device),
                d3d_driver_type: D3D_DRIVER_TYPE_UNKNOWN,
                display_is_owned: false,
            })
        }
    }

    /// Returns the display server connection that this device was created with.
    #[inline]
    pub fn connection(&self) -> Connection {
        Connection
    }

    /// Returns the adapter that this device was created with.
    pub fn adapter(&self) -> Adapter {
        unsafe {
            let mut dxgi_device: *mut IDXGIDevice = ptr::null_mut();
            let result = (*self.d3d11_device).QueryInterface(
                &IDXGIDevice::uuidof(),
                &mut dxgi_device as *mut *mut IDXGIDevice as *mut *mut c_void,
            );
            assert!(winerror::SUCCEEDED(result));
            let dxgi_device = ComPtr::from_raw(dxgi_device);

            let mut dxgi_adapter = ptr::null_mut();
            let result = (*dxgi_device).GetAdapter(&mut dxgi_adapter);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            Adapter {
                dxgi_adapter,
                d3d_driver_type: self.d3d_driver_type,
            }
        }
    }

    /// Returns the underlying native device type.
    ///
    /// The reference count on the underlying Direct3D device is increased before returning it.
    #[inline]
    pub fn native_device(&self) -> NativeDevice {
        NativeDevice {
            egl_display: self.egl_display,
            d3d11_device: self.d3d11_device.clone().into_raw(),
            d3d_driver_type: self.d3d_driver_type,
        }
    }

    /// Returns the OpenGL API flavor that this device supports (OpenGL or OpenGL ES).
    #[inline]
    pub fn gl_api(&self) -> GLApi {
        GLApi::GLES
    }

    /// Creates a context descriptor with the given attributes.
    ///
    /// Context descriptors are local to this device.
    #[inline]
    pub fn create_context_descriptor(
        &self,
        attributes: &ContextAttributes,
    ) -> Result<ContextDescriptor, Error> {
        unsafe {
            ContextDescriptor::new(
                self.egl_display,
                attributes,
                &[
                    egl::BIND_TO_TEXTURE_RGBA as EGLint,
                    1 as EGLint,
                    egl::SURFACE_TYPE as EGLint,
                    egl::PBUFFER_BIT as EGLint,
                    egl::RENDERABLE_TYPE as EGLint,
                    egl::OPENGL_ES2_BIT as EGLint,
                ],
            )
        }
    }

    /// Creates a new OpenGL context.
    ///
    /// The context initially has no surface attached. Until a surface is bound to it, rendering
    /// commands will fail or have no effect.
    pub fn create_context(
        &self,
        descriptor: &ContextDescriptor,
        share_with: Option<&Context>,
    ) -> Result<Context, Error> {
        let (egl_context, id) = {
            let mut next_context_id_lock = CREATE_CONTEXT_MUTEX.lock().unwrap();
            let egl_context = unsafe {
                context::create_context(
                    self.egl_display,
                    descriptor,
                    share_with.map_or(egl::NO_CONTEXT, |ctx| ctx.egl_context),
                    self.gl_api(),
                )?
            };
            next_context_id_lock.0 += 1;
            (egl_context, *next_context_id_lock)
        };

        unsafe {
            EGL_FUNCTIONS.with(|egl| {
                let result = egl.MakeCurrent(
                    self.egl_display,
                    egl::NO_SURFACE,
                    egl::NO_SURFACE,
                    egl_context,
                );
                if result == egl::FALSE {
                    return Err(Error::MakeCurrentFailed(
                        egl.GetError().to_windowing_api_error(),
                    ));
                }
                Ok(())
            })?;
        }

        let context = Context {
            egl_context,
            id,
            framebuffer: Framebuffer::None,
            context_is_owned: true,
            gl: unsafe { Gl::from_loader_function(context::get_proc_address) },
        };
        Ok(context)
    }

    /// Wraps a native `EGLContext` in a context object.
    ///
    /// The underlying `EGLContext` is not retained, as there is no way to do this in the EGL API.
    /// Therefore, it is the caller's responsibility to keep it alive as long as this `Context`
    /// remains alive.
    pub unsafe fn create_context_from_native_context(
        &self,
        native_context: NativeContext,
    ) -> Result<Context, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Create the context.
        let context = Context {
            egl_context: native_context.egl_context,
            id: *next_context_id,
            framebuffer: Framebuffer::External(ExternalEGLSurfaces {
                draw: native_context.egl_draw_surface,
                read: native_context.egl_read_surface,
            }),
            context_is_owned: false,
            gl: Gl::from_loader_function(context::get_proc_address),
        };
        next_context_id.0 += 1;

        Ok(context)
    }

    /// Destroys a context.
    ///
    /// The context must have been created on this device.
    pub fn destroy_context(&self, context: &mut Context) -> Result<(), Error> {
        if context.egl_context == egl::NO_CONTEXT {
            return Ok(());
        }

        if let Ok(Some(mut surface)) = self.unbind_surface_from_context(context) {
            self.destroy_surface(context, &mut surface)?;
        }

        EGL_FUNCTIONS.with(|egl| unsafe {
            egl.MakeCurrent(
                self.egl_display,
                egl::NO_SURFACE,
                egl::NO_SURFACE,
                egl::NO_CONTEXT,
            );

            if context.context_is_owned {
                let result = egl.DestroyContext(self.egl_display, context.egl_context);
                assert_ne!(result, egl::FALSE);
            }

            context.egl_context = egl::NO_CONTEXT;
        });

        Ok(())
    }

    /// Returns the descriptor that this context was created with.
    pub fn context_descriptor(&self, context: &Context) -> ContextDescriptor {
        unsafe {
            ContextDescriptor::from_egl_context(&context.gl, self.egl_display, context.egl_context)
        }
    }

    /// Makes the context the current OpenGL context for this thread.
    ///
    /// After calling this function, it is valid to use OpenGL rendering commands.
    pub fn make_context_current(&self, context: &Context) -> Result<(), Error> {
        unsafe {
            let (egl_draw_surface, egl_read_surface) = match context.framebuffer {
                Framebuffer::Surface(ref surface) => (surface.egl_surface, surface.egl_surface),
                Framebuffer::None => (egl::NO_SURFACE, egl::NO_SURFACE),
                Framebuffer::External(ref surfaces) => (surfaces.draw, surfaces.read),
            };

            EGL_FUNCTIONS.with(|egl| {
                let result = egl.MakeCurrent(
                    self.egl_display,
                    egl_draw_surface,
                    egl_read_surface,
                    context.egl_context,
                );
                if result == egl::FALSE {
                    let err = egl.GetError().to_windowing_api_error();
                    return Err(Error::MakeCurrentFailed(err));
                }
                Ok(())
            })
        }
    }

    /// Removes the current OpenGL context from this thread.
    ///
    /// After calling this function, OpenGL rendering commands will fail until a new context is
    /// made current.
    pub fn make_no_context_current(&self) -> Result<(), Error> {
        unsafe { context::make_no_context_current(self.egl_display) }
    }

    pub(crate) fn temporarily_make_context_current(
        &self,
        context: &Context,
    ) -> Result<CurrentContextGuard, Error> {
        let guard = CurrentContextGuard::new();
        self.make_context_current(context)?;
        Ok(guard)
    }

    pub(crate) fn context_is_current(&self, context: &Context) -> bool {
        EGL_FUNCTIONS.with(|egl| unsafe { egl.GetCurrentContext() == context.egl_context })
    }

    /// Returns the attributes that the context descriptor was created with.
    #[inline]
    pub fn context_descriptor_attributes(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> ContextAttributes {
        unsafe { context_descriptor.attributes(self.egl_display) }
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
        context::get_proc_address(symbol_name)
    }

    #[inline]
    pub(crate) fn context_descriptor_to_egl_config(
        &self,
        context_descriptor: &ContextDescriptor,
    ) -> EGLConfig {
        unsafe { context::egl_config_from_id(self.egl_display, context_descriptor.egl_config_id) }
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
            Framebuffer::External(_) => return Err((Error::ExternalRenderTarget, surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, surface)),
        }

        // If the surface is synchronized with GLFinish, then finish.
        // FIXME(pcwalton): Is this necessary and sufficient?
        if surface.uses_gl_finish() {
            if let Ok(_guard) = self.temporarily_make_context_current(context) {
                unsafe {
                    context.gl.finish();
                }
            }
        }

        let is_current = self.context_is_current(context);

        match surface.win32_objects {
            Win32Objects::Pbuffer {
                synchronization: Synchronization::KeyedMutex(ref keyed_mutex),
                ..
            } => unsafe {
                let result = keyed_mutex.AcquireSync(0, INFINITE);
                assert_eq!(result, S_OK);
            },
            _ => {}
        }

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
        match context.framebuffer {
            Framebuffer::None => return Ok(None),
            Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(_) => {}
        }

        let surface = match mem::replace(&mut context.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => surface,
            Framebuffer::None | Framebuffer::External(_) => unreachable!(),
        };

        match surface.win32_objects {
            Win32Objects::Pbuffer {
                synchronization: Synchronization::KeyedMutex(ref keyed_mutex),
                ..
            } => unsafe {
                let result = keyed_mutex.ReleaseSync(0);
                assert_eq!(result, S_OK);
            },
            _ => {}
        }

        Ok(Some(surface))
    }

    /// Displays the contents of the currently bound surface to the screen, if
    /// it is a widget surface.
    ///
    /// Widget surfaces are internally double-buffered, so changes to them don't
    /// show up in their associated widgets until this method is called.
    pub fn present_bound_surface(&self, context: &mut Context) -> Result<(), Error> {
        match &context.framebuffer {
            Framebuffer::Surface(surface) => surface.present(self),
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
            Framebuffer::External(_) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(self.surface_info(surface))),
        }
    }

    /// Given a context, returns its underlying EGL context and attached surfaces.
    pub fn native_context(&self, context: &Context) -> NativeContext {
        let (egl_draw_surface, egl_read_surface) = match context.framebuffer {
            Framebuffer::Surface(Surface { egl_surface, .. }) => (egl_surface, egl_surface),
            Framebuffer::External(ExternalEGLSurfaces { draw, read }) => (draw, read),
            Framebuffer::None => (egl::NO_SURFACE, egl::NO_SURFACE),
        };

        NativeContext {
            egl_context: context.egl_context,
            egl_draw_surface,
            egl_read_surface,
        }
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
            SurfaceType::Generic { ref size } => self.create_pbuffer_surface(context, size, None),
            SurfaceType::Widget { ref native_widget } => {
                self.create_window_surface(context, native_widget)
            }
        }
    }

    #[allow(non_snake_case)]
    fn create_pbuffer_surface(
        &self,
        context: &Context,
        size: &Size2D<i32>,
        texture: Option<ComPtr<d3d11::ID3D11Texture2D>>,
    ) -> Result<Surface, Error> {
        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        unsafe {
            let attributes = [
                egl::WIDTH as EGLint,
                size.width as EGLint,
                egl::HEIGHT as EGLint,
                size.height as EGLint,
                egl::TEXTURE_FORMAT as EGLint,
                egl::TEXTURE_RGBA as EGLint,
                egl::TEXTURE_TARGET as EGLint,
                egl::TEXTURE_2D as EGLint,
                egl::NONE as EGLint,
                0,
                0,
                0,
            ];

            EGL_FUNCTIONS.with(|egl| {
                let egl_surface = if let Some(ref texture) = texture {
                    let surface = egl.CreatePbufferFromClientBuffer(
                        self.egl_display,
                        EGL_D3D_TEXTURE_ANGLE,
                        texture.as_raw() as *const _,
                        egl_config,
                        attributes.as_ptr(),
                    );
                    assert_ne!(surface, egl::NO_SURFACE);
                    surface
                } else {
                    let surface =
                        egl.CreatePbufferSurface(self.egl_display, egl_config, attributes.as_ptr());
                    assert_ne!(surface, egl::NO_SURFACE);
                    surface
                };

                let eglQuerySurfacePointerANGLE =
                    EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE.expect(
                        "Where's the `EGL_ANGLE_query_surface_pointer` \
                                                 extension?",
                    );

                let mut share_handle = INVALID_HANDLE_VALUE;
                let result = eglQuerySurfacePointerANGLE(
                    self.egl_display,
                    egl_surface,
                    EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE as EGLint,
                    &mut share_handle,
                );
                assert_ne!(result, egl::FALSE);
                assert_ne!(share_handle, INVALID_HANDLE_VALUE);

                // `mozangle` builds ANGLE with keyed mutexes for sharing. Use the
                // `EGL_ANGLE_keyed_mutex` extension to fetch the keyed mutex so we can grab it.
                let mut keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
                let result = eglQuerySurfacePointerANGLE(
                    self.egl_display,
                    egl_surface,
                    EGL_DXGI_KEYED_MUTEX_ANGLE as EGLint,
                    &mut keyed_mutex as *mut *mut IDXGIKeyedMutex as *mut *mut c_void,
                );
                let synchronization = if result != egl::FALSE && !keyed_mutex.is_null() {
                    let keyed_mutex = ComPtr::from_raw(keyed_mutex);
                    keyed_mutex.AddRef();
                    Synchronization::KeyedMutex(keyed_mutex)
                } else if texture.is_none() {
                    Synchronization::GLFinish
                } else {
                    Synchronization::None
                };

                Ok(Surface {
                    egl_surface,
                    size: *size,
                    context_id: context.id,
                    context_descriptor,
                    win32_objects: Win32Objects::Pbuffer {
                        share_handle,
                        synchronization,
                        texture,
                    },
                })
            })
        }
    }

    /// Given a D3D11 texture, create a surface that wraps that texture. This method is unsafe
    /// in that the resulting surface is only valid on the current thread.
    pub unsafe fn create_surface_from_texture(
        &self,
        context: &Context,
        size: &Size2D<i32>,
        texture: ComPtr<d3d11::ID3D11Texture2D>,
    ) -> Result<Surface, Error> {
        self.create_pbuffer_surface(context, size, Some(texture))
    }

    fn create_window_surface(
        &self,
        context: &Context,
        native_widget: &NativeWidget,
    ) -> Result<Surface, Error> {
        let context_descriptor = self.context_descriptor(context);
        let egl_config = self.context_descriptor_to_egl_config(&context_descriptor);

        unsafe {
            EGL_FUNCTIONS.with(|egl| {
                let attributes = [egl::NONE as EGLint];
                let egl_surface = egl.CreateWindowSurface(
                    self.egl_display,
                    egl_config,
                    native_widget.egl_native_window,
                    attributes.as_ptr(),
                );
                assert_ne!(egl_surface, egl::NO_SURFACE);

                let mut width = 0;
                let mut height = 0;
                egl.QuerySurface(
                    self.egl_display,
                    egl_surface,
                    egl::WIDTH as EGLint,
                    &mut width,
                );
                egl.QuerySurface(
                    self.egl_display,
                    egl_surface,
                    egl::HEIGHT as EGLint,
                    &mut height,
                );
                assert_ne!(width, 0);
                assert_ne!(height, 0);

                Ok(Surface {
                    egl_surface,
                    size: Size2D::new(width, height),
                    context_id: context.id,
                    context_descriptor,
                    win32_objects: Win32Objects::Window,
                })
            })
        }
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
    pub fn create_surface_texture(
        &self,
        context: &mut Context,
        surface: Surface,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        let share_handle = match surface.win32_objects {
            Win32Objects::Window => return Err((Error::WidgetAttached, surface)),
            Win32Objects::Pbuffer { share_handle, .. } => share_handle,
        };

        let local_egl_config = self.context_descriptor_to_egl_config(&surface.context_descriptor);
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                // First, create an EGL surface local to this thread.
                let pbuffer_attributes = [
                    egl::WIDTH as EGLint,
                    surface.size.width,
                    egl::HEIGHT as EGLint,
                    surface.size.height,
                    egl::TEXTURE_FORMAT as EGLint,
                    egl::TEXTURE_RGBA as EGLint,
                    egl::TEXTURE_TARGET as EGLint,
                    egl::TEXTURE_2D as EGLint,
                    egl::NONE as EGLint,
                    0,
                    0,
                    0,
                ];

                let local_egl_surface = egl.CreatePbufferFromClientBuffer(
                    self.egl_display,
                    EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                    share_handle,
                    local_egl_config,
                    pbuffer_attributes.as_ptr(),
                );
                if local_egl_surface == egl::NO_SURFACE {
                    let windowing_api_error = egl.GetError().to_windowing_api_error();
                    return Err((Error::SurfaceImportFailed(windowing_api_error), surface));
                }

                let mut local_keyed_mutex: *mut IDXGIKeyedMutex = ptr::null_mut();
                let eglQuerySurfacePointerANGLE =
                    EGL_EXTENSION_FUNCTIONS.QuerySurfacePointerANGLE.unwrap();
                let result = eglQuerySurfacePointerANGLE(
                    self.egl_display,
                    local_egl_surface,
                    EGL_DXGI_KEYED_MUTEX_ANGLE as EGLint,
                    &mut local_keyed_mutex as *mut *mut IDXGIKeyedMutex as *mut *mut c_void,
                );
                let local_keyed_mutex = if result != egl::FALSE && !local_keyed_mutex.is_null() {
                    let local_keyed_mutex = ComPtr::from_raw(local_keyed_mutex);
                    local_keyed_mutex.AddRef();

                    let result = local_keyed_mutex.AcquireSync(0, INFINITE);
                    assert_eq!(result, S_OK);

                    Some(local_keyed_mutex)
                } else {
                    None
                };
                self.create_surface_texture_from_local_surface(
                    context,
                    surface,
                    local_egl_surface,
                    local_keyed_mutex,
                )
            }
        })
    }

    fn create_surface_texture_from_local_surface(
        &self,
        context: &Context,
        surface: Surface,
        local_egl_surface: EGLSurface,
        local_keyed_mutex: Option<ComPtr<IDXGIKeyedMutex>>,
    ) -> Result<SurfaceTexture, (Error, Surface)> {
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                let _guard = self.temporarily_make_context_current(context);

                let gl = &context.gl;
                // Then bind that surface to the texture.
                let texture = gl.create_texture().unwrap();

                gl.bind_texture(gl::TEXTURE_2D, Some(texture));
                if egl.BindTexImage(self.egl_display, local_egl_surface, egl::BACK_BUFFER as _)
                    == egl::FALSE
                {
                    let windowing_api_error = egl.GetError().to_windowing_api_error();
                    return Err((
                        Error::SurfaceTextureCreationFailed(windowing_api_error),
                        surface,
                    ));
                }

                // Initialize the texture, for convenience.
                gl.tex_parameter_i32(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
                gl.tex_parameter_i32(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
                gl.tex_parameter_i32(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
                gl.tex_parameter_i32(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);

                gl.bind_texture(gl::TEXTURE_2D, None);
                debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

                Ok(SurfaceTexture {
                    surface,
                    local_egl_surface,
                    local_keyed_mutex,
                    gl_texture: Some(texture),
                    phantom: PhantomData,
                })
            }
        })
    }

    /// Given a D3D11 texture, create a surface texture that wraps that texture. This method is unsafe
    /// in that the resulting surface is only valid on the current thread, for the lifetime of `texture`.
    /// It is the caller's responsibility to ensure that `texture` is not freed while the `SurfaceTexture` is live.
    pub unsafe fn create_surface_texture_from_texture(
        &self,
        context: &mut Context,
        size: &Size2D<i32>,
        texture: ComPtr<d3d11::ID3D11Texture2D>,
    ) -> Result<SurfaceTexture, Error> {
        let surface = self.create_pbuffer_surface(context, size, Some(texture))?;
        let local_egl_surface = surface.egl_surface;
        self.create_surface_texture_from_local_surface(context, surface, local_egl_surface, None)
            .map_err(|(err, mut surface)| {
                let _ = self.destroy_surface(context, &mut surface);
                err
            })
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
        if context.id != surface.context_id {
            return Err(Error::IncompatibleSurface);
        }

        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                // If the surface is currently bound, unbind it.
                if egl.GetCurrentSurface(egl::READ as EGLint) == surface.egl_surface
                    || egl.GetCurrentSurface(egl::DRAW as EGLint) == surface.egl_surface
                {
                    self.make_no_context_current()?;
                }

                egl.DestroySurface(self.egl_display, surface.egl_surface);
                surface.egl_surface = egl::NO_SURFACE;
                if let Win32Objects::Pbuffer {
                    ref mut texture, ..
                } = surface.win32_objects
                {
                    texture.take();
                }
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
    pub fn destroy_surface_texture(
        &self,
        context: &mut Context,
        mut surface_texture: SurfaceTexture,
    ) -> Result<Surface, (Error, SurfaceTexture)> {
        unsafe {
            if let Some(texture) = surface_texture.gl_texture.take() {
                context.gl.delete_texture(texture);
            }

            if let Some(ref local_keyed_mutex) = surface_texture.local_keyed_mutex {
                let result = local_keyed_mutex.ReleaseSync(0);
                assert_eq!(result, S_OK);
            }

            EGL_FUNCTIONS.with(|egl| {
                egl.ReleaseTexImage(
                    self.egl_display,
                    surface_texture.local_egl_surface,
                    egl::BACK_BUFFER as _,
                );
                egl.DestroySurface(self.egl_display, surface_texture.local_egl_surface);
            })
        }

        Ok(surface_texture.surface)
    }

    /// Returns the OpenGL texture target needed to read from this surface texture.
    ///
    /// This will be `GL_TEXTURE_2D` or `GL_TEXTURE_RECTANGLE`, depending on platform.
    #[inline]
    pub fn surface_gl_texture_target(&self) -> u32 {
        SURFACE_GL_TEXTURE_TARGET
    }

    /// Returns a pointer to the underlying surface data for reading or writing by the CPU.
    #[inline]
    pub fn lock_surface_data<'s>(
        &self,
        _surface: &'s mut Surface,
    ) -> Result<SurfaceDataGuard<'s>, Error> {
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
        surface.present(self)
    }

    /// Resizes a widget surface.
    pub fn resize_surface(
        &self,
        _context: &Context,
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
            framebuffer_object: None,
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

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            if self.display_is_owned {
                EGL_FUNCTIONS.with(|egl| {
                    let result = egl.Terminate(self.egl_display);
                    assert_ne!(result, egl::FALSE);
                    self.egl_display = egl::NO_DISPLAY;
                })
            }
        }
    }
}
