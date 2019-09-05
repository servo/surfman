//! A thread-local handle to the device.

use crate::egl::types::{EGLAttrib, EGLBoolean, EGLConfig, EGLContext, EGLDeviceEXT, EGLDisplay};
use crate::egl::types::{EGLSurface, EGLenum, EGLint};
use crate::egl;
use std::os::raw::c_void;

struct EGLExtensionFunctions {
    CreateDeviceANGLE: extern "C" fn(device_type: EGLint,
                                     native_device: *mut c_void,
                                     attrib_list: *const EGLAttrib)
                                     -> EGLDeviceEXT;
    QuerySurfacePointerANGLE: extern "C" fn(dpy: EGLDisplay,
                                            surface: EGLSurface,
                                            attribute: EGLint,
                                            value: *mut *mut c_void)
                                            -> EGLBoolean;
}

lazy_static! {
    static ref EGL_EXTENSION_FUNCTIONS: EGLExtensionFunctions = {
        unsafe {
            EGLExtensionFunctions {
                CreateDeviceANGLE: lookup_egl_extension(b"eglCreateDeviceANGLE\0"),
                QuerySurfacePointerANGLE: lookup_egl_extension(b"eglQuerySurfacePointerANGLE\0"),
            }
        }
    };
}

pub struct Device {
    egl_device: EGLDeviceEXT,
    egl_display: EGLDisplay,
    surfaces: Vec<Surface>,
    owned_by_us: bool,
}

impl Device {
    #[inline]
    pub fn new(adapter: &Adapter) -> Result<Device, Error> {
        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let mut d3d11_device_context = ptr::null_mut();
            let result = D3D11CreateDevice(adapter.dxgi_adapter.as_ptr(),
                                           adapter.driver_type,
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
            debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
            let d3d11_device = ComPtr::from_raw(d3d11_device);
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let egl_device = (*eglCreateDeviceANGLE)(EGL_D3D11_DEVICE_ANGLE,
                                                     d3d11_device.as_raw() as *mut c_void,
                                                     ptr::null_mut());
            assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

            let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
            let egl_display = egl::GetPlatformDisplay(EGL_PLATFORM_DEVICE_EXT,
                                                      egl_device as *mut c_void,
                                                      &attribs[0]);
            assert_ne!(egl_display, egl::NO_DISPLAY);

            let (mut major_version, mut minor_version) = (0, 0);
            let result = egl::Initialize(egl_display, &mut major_version, &mut minor_version);
            assert_ne!(result, egl::FALSE);

            Ok(Device {
        }
    }

    #[inline]
    pub fn adapter(&self) -> Adapter {
        Adapter
    }
}

unsafe fn lookup_egl_extension(name: &'static [u8]) -> *const c_void {
    let f = egl::GetProcAddress(&name[0] as *const u8 as *const c_char);
    assert_ne!(f as usize, 0);
    f
}