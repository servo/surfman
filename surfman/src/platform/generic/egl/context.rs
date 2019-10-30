// surfman/surfman/src/platform/generic/egl/context.rs
//
//! Functionality common to backends using EGL contexts.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion};
use crate::egl::types::{EGLConfig, EGLContext, EGLDisplay, EGLSurface, EGLint};
use crate::egl;
use super::device::EGL_FUNCTIONS;
use super::error::ToWindowingApiError;

use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;

#[allow(dead_code)]
const DUMMY_PBUFFER_SIZE: EGLint = 16;
const RGB_CHANNEL_BIT_DEPTH: EGLint = 8;

pub(crate) trait NativeContext {
    fn egl_context(&self) -> EGLContext;
    fn is_destroyed(&self) -> bool;
    unsafe fn destroy(&mut self, egl_display: EGLDisplay);
}

#[derive(Clone)]
pub struct ContextDescriptor {
    pub(crate) egl_config_id: EGLint,
    pub(crate) egl_context_client_version: EGLint,
}

#[must_use]
pub(crate) struct CurrentContextGuard {
    egl_display: EGLDisplay,
    old_egl_draw_surface: EGLSurface,
    old_egl_read_surface: EGLSurface,
    old_egl_context: EGLContext,
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                if self.egl_display != egl::NO_DISPLAY {
                    egl.MakeCurrent(self.egl_display,
                                    self.old_egl_draw_surface,
                                    self.old_egl_read_surface,
                                    self.old_egl_context);
                }
            }
        })
    }
}

impl ContextDescriptor {
    pub(crate) unsafe fn new(egl_display: EGLDisplay,
                             attributes: &ContextAttributes,
                             extra_config_attributes: &[EGLint])
                             -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;
        let alpha_size   = if flags.contains(ContextAttributeFlags::ALPHA)   { 8  } else { 0 };
        let depth_size   = if flags.contains(ContextAttributeFlags::DEPTH)   { 24 } else { 0 };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) { 8  } else { 0 };

        // Create required config attributes.
        let required_config_attributes = [
            egl::RED_SIZE as EGLint,    RGB_CHANNEL_BIT_DEPTH,
            egl::GREEN_SIZE as EGLint,  RGB_CHANNEL_BIT_DEPTH,
            egl::BLUE_SIZE as EGLint,   RGB_CHANNEL_BIT_DEPTH,
            egl::ALPHA_SIZE as EGLint,  alpha_size,
        ];

        // Create config attributes.
        let mut requested_config_attributes = required_config_attributes.to_vec();
        requested_config_attributes.extend_from_slice(&[
            egl::DEPTH_SIZE as EGLint,      depth_size,
            egl::STENCIL_SIZE as EGLint,    stencil_size,
        ]);
        requested_config_attributes.extend_from_slice(extra_config_attributes);
        requested_config_attributes.extend_from_slice(&[egl::NONE as EGLint, 0, 0, 0]);

        EGL_FUNCTIONS.with(|egl| {
            // See how many applicable configs there are.
            let mut config_count = 0;
            let result = egl.ChooseConfig(egl_display,
                                          requested_config_attributes.as_ptr(),
                                          ptr::null_mut(),
                                          0,
                                          &mut config_count);
            if result == egl::FALSE {
                let err = egl.GetError().to_windowing_api_error();
                return Err(Error::PixelFormatSelectionFailed(err));
            }
            if config_count == 0 {
                return Err(Error::NoPixelFormatFound);
            }

            // Enumerate all those configs.
            let mut configs = vec![ptr::null(); config_count as usize];
            let mut real_config_count = config_count;
            let result = egl.ChooseConfig(egl_display,
                                          requested_config_attributes.as_ptr(),
                                          configs.as_mut_ptr(),
                                          config_count,
                                          &mut real_config_count);
            if result == egl::FALSE {
                let err = egl.GetError().to_windowing_api_error();
                return Err(Error::PixelFormatSelectionFailed(err));
            }

            // Sanitize configs.
            let egl_config = configs.into_iter().filter(|&egl_config| {
                required_config_attributes.chunks(2).all(|pair| {
                    get_config_attr(egl_display, egl_config, pair[0]) == pair[1]
                })
            }).next();
            let egl_config = match egl_config {
                None => return Err(Error::NoPixelFormatFound),
                Some(egl_config) => egl_config,
            };

            // Get the config ID and version.
            let egl_config_id = get_config_attr(egl_display, egl_config, egl::CONFIG_ID as EGLint);
            let egl_context_client_version = attributes.version.major as EGLint;

            Ok(ContextDescriptor { egl_config_id, egl_context_client_version })
        })
    }

    pub(crate) unsafe fn from_egl_context(egl_display: EGLDisplay, egl_context: EGLContext)
                                          -> ContextDescriptor {
        let egl_config_id = get_context_attr(egl_display, egl_context, egl::CONFIG_ID as EGLint);
        let egl_context_client_version = get_context_attr(egl_display,
                                                          egl_context,
                                                          egl::CONTEXT_CLIENT_VERSION as EGLint);
        ContextDescriptor { egl_config_id, egl_context_client_version }
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn to_egl_config(&self, egl_display: EGLDisplay) -> EGLConfig {
        let config_attributes = [
            egl::CONFIG_ID as EGLint,   self.egl_config_id,
            egl::NONE as EGLint,        0,
            0,                          0,
        ];

        EGL_FUNCTIONS.with(|egl| {
            let (mut config, mut config_count) = (ptr::null(), 0);
            let result = egl.ChooseConfig(egl_display,
                                          config_attributes.as_ptr(),
                                          &mut config,
                                          1,
                                          &mut config_count);
            assert_ne!(result, egl::FALSE);
            assert!(config_count > 0);
            config
        })
    }

    pub(crate) unsafe fn attributes(&self, egl_display: EGLDisplay) -> ContextAttributes {
        let egl_config = egl_config_from_id(egl_display, self.egl_config_id);

        let alpha_size = get_config_attr(egl_display, egl_config, egl::ALPHA_SIZE as EGLint);
        let depth_size = get_config_attr(egl_display, egl_config, egl::DEPTH_SIZE as EGLint);
        let stencil_size = get_config_attr(egl_display, egl_config, egl::STENCIL_SIZE as EGLint);

        // Convert to `surfman` context attribute flags.
        let mut attribute_flags = ContextAttributeFlags::empty();
        attribute_flags.set(ContextAttributeFlags::ALPHA, alpha_size != 0);
        attribute_flags.set(ContextAttributeFlags::DEPTH, depth_size != 0);
        attribute_flags.set(ContextAttributeFlags::STENCIL, stencil_size != 0);

        // Create appropriate context attributes.
        ContextAttributes {
            flags: attribute_flags,
            version: GLVersion::new(self.egl_context_client_version as u8, 0),
        }
    }
}

impl CurrentContextGuard {
    pub(crate) fn new() -> CurrentContextGuard {
        EGL_FUNCTIONS.with(|egl| {
            unsafe {
                CurrentContextGuard {
                    egl_display: egl.GetCurrentDisplay(),
                    old_egl_draw_surface: egl.GetCurrentSurface(egl::DRAW as EGLint),
                    old_egl_read_surface: egl.GetCurrentSurface(egl::READ as EGLint),
                    old_egl_context: egl.GetCurrentContext(),
                }
            }
        })
    }
}

pub(crate) struct OwnedEGLContext {
    pub(crate) egl_context: EGLContext,
}

impl NativeContext for OwnedEGLContext {
    #[inline]
    fn egl_context(&self) -> EGLContext {
        self.egl_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_context == egl::NO_CONTEXT
    }

    unsafe fn destroy(&mut self, egl_display: EGLDisplay) {
        assert!(!self.is_destroyed());

        EGL_FUNCTIONS.with(|egl| {
            egl.MakeCurrent(egl_display, egl::NO_SURFACE, egl::NO_SURFACE, egl::NO_CONTEXT);
            let result = egl.DestroyContext(egl_display, self.egl_context);
            assert_ne!(result, egl::FALSE);
            self.egl_context = egl::NO_CONTEXT;
        })
    }
}

pub(crate) struct UnsafeEGLContextRef {
    pub(crate) egl_context: EGLContext,
}

impl NativeContext for UnsafeEGLContextRef {
    #[inline]
    fn egl_context(&self) -> EGLContext {
        self.egl_context
    }

    #[inline]
    fn is_destroyed(&self) -> bool {
        self.egl_context == egl::NO_CONTEXT
    }

    unsafe fn destroy(&mut self, _: EGLDisplay) {
        assert!(!self.is_destroyed());
        self.egl_context = egl::NO_CONTEXT;
    }
}

pub(crate) unsafe fn create_context(egl_display: EGLDisplay, descriptor: &ContextDescriptor)
                                    -> Result<EGLContext, Error> {
    let egl_config = egl_config_from_id(egl_display, descriptor.egl_config_id);
    let egl_context_client_version = descriptor.egl_context_client_version;

    // Include some extra zeroes to work around broken implementations.
    //
    // FIXME(pcwalton): Which implementations are those? (This is copied from Gecko.)
    let egl_context_attributes = [
        egl::CONTEXT_CLIENT_VERSION as EGLint, egl_context_client_version,
        egl::NONE as EGLint, 0,
        0, 0,
    ];

    EGL_FUNCTIONS.with(|egl| {
        let egl_context = egl.CreateContext(egl_display,
                                            egl_config,
                                            egl::NO_CONTEXT,
                                            egl_context_attributes.as_ptr());
        if egl_context == egl::NO_CONTEXT {
            let err = egl.GetError().to_windowing_api_error();
            return Err(Error::ContextCreationFailed(err));
        }

        Ok(egl_context)
    })
}

pub(crate) unsafe fn make_no_context_current(egl_display: EGLDisplay) -> Result<(), Error> {
    EGL_FUNCTIONS.with(|egl| {
        let result = egl.MakeCurrent(egl_display,
                                     egl::NO_SURFACE,
                                     egl::NO_SURFACE,
                                     egl::NO_CONTEXT);
        if result == egl::FALSE {
            let err = egl.GetError().to_windowing_api_error();
            return Err(Error::MakeCurrentFailed(err));
        }
        Ok(())
    })
}

pub(crate) unsafe fn get_config_attr(egl_display: EGLDisplay, egl_config: EGLConfig, attr: EGLint)
                                     -> EGLint {
    EGL_FUNCTIONS.with(|egl| {
        let mut value = 0;
        let result = egl.GetConfigAttrib(egl_display, egl_config, attr, &mut value);
        assert_ne!(result, egl::FALSE);
        value
    })
}

pub(crate) unsafe fn get_context_attr(egl_display: EGLDisplay,
                                      egl_context: EGLContext,
                                      attr: EGLint)
                                      -> EGLint {
    EGL_FUNCTIONS.with(|egl| {
        let mut value = 0;
        let result = egl.QueryContext(egl_display, egl_context, attr, &mut value);
        assert_ne!(result, egl::FALSE);
        value
    })
}

pub(crate) unsafe fn egl_config_from_id(egl_display: EGLDisplay, egl_config_id: EGLint)
                                        -> EGLConfig {
    let config_attributes = [
        egl::CONFIG_ID as EGLint,   egl_config_id,
        egl::NONE as EGLint,        0,
        0,                          0,
    ];

    EGL_FUNCTIONS.with(|egl| {
        let (mut config, mut config_count) = (ptr::null(), 0);
        let result = egl.ChooseConfig(egl_display,
                                      config_attributes.as_ptr(),
                                      &mut config,
                                      1,
                                      &mut config_count);
        assert_ne!(result, egl::FALSE);
        assert!(config_count > 0);
        config
    })
}

pub(crate) fn get_proc_address(symbol_name: &str) -> *const c_void {
    EGL_FUNCTIONS.with(|egl| {
        unsafe {
            let symbol_name: CString = CString::new(symbol_name).unwrap();
            egl.GetProcAddress(symbol_name.as_ptr() as *const u8 as *const c_char) as *const c_void
        }
    })
}

// Creates and returns a dummy pbuffer surface for the given context. This is used as the default
// framebuffer on some backends.
#[allow(dead_code)]
pub(crate) unsafe fn create_dummy_pbuffer(egl_display: EGLDisplay, egl_context: EGLContext)
                                          -> EGLSurface {
    let egl_config_id = get_context_attr(egl_display, egl_context, egl::CONFIG_ID as EGLint);
    let egl_config = egl_config_from_id(egl_display, egl_config_id);

    let pbuffer_attributes = [
        egl::WIDTH as EGLint,   DUMMY_PBUFFER_SIZE,
        egl::HEIGHT as EGLint,  DUMMY_PBUFFER_SIZE,
        egl::NONE as EGLint,    0,
        0,                      0,
    ];

    EGL_FUNCTIONS.with(|egl| {
        let pbuffer = egl.CreatePbufferSurface(egl_display,
                                               egl_config,
                                               pbuffer_attributes.as_ptr());
        assert_ne!(pbuffer, egl::NO_SURFACE);
        pbuffer
    })
}

