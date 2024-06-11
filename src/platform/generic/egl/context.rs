// surfman/surfman/src/platform/generic/egl/context.rs
//
//! Functionality common to backends using EGL contexts.

use super::device::EGL_FUNCTIONS;
use super::error::ToWindowingApiError;
use super::ffi::EGL_CONTEXT_OPENGL_PROFILE_MASK;
use super::ffi::{EGL_CONTEXT_MINOR_VERSION_KHR, EGL_CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT};
use super::surface::{EGLBackedSurface, ExternalEGLSurfaces};
use crate::context::{self, CREATE_CONTEXT_MUTEX};
use crate::egl;
use crate::egl::types::{EGLConfig, EGLContext, EGLDisplay, EGLSurface, EGLint};
use crate::surface::Framebuffer;
use crate::{ContextAttributeFlags, ContextAttributes, ContextID, Error, GLApi, GLVersion};
use crate::{Gl, SurfaceInfo};

use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::thread;

#[allow(dead_code)]
const DUMMY_PBUFFER_SIZE: EGLint = 16;
const RGB_CHANNEL_BIT_DEPTH: EGLint = 8;

pub(crate) struct EGLBackedContext {
    pub(crate) egl_context: EGLContext,
    pub(crate) id: ContextID,
    framebuffer: Framebuffer<EGLBackedSurface, ExternalEGLSurfaces>,
    context_is_owned: bool,
}

/// Wrapper for a native `EGLContext`.
#[derive(Clone, Copy)]
pub struct NativeContext {
    /// The EGL context.
    pub egl_context: EGLContext,
    /// The EGL read surface that is to be attached to that context.
    pub egl_read_surface: EGLSurface,
    /// The EGL draw surface that is to be attached to that context.
    pub egl_draw_surface: EGLSurface,
}

/// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
///
/// These are local to a device.
#[derive(Clone)]
pub struct ContextDescriptor {
    pub(crate) egl_config_id: EGLint,
    pub(crate) gl_version: GLVersion,
    pub(crate) compatibility_profile: bool,
}

#[must_use]
pub(crate) struct CurrentContextGuard {
    egl_display: EGLDisplay,
    old_egl_draw_surface: EGLSurface,
    old_egl_read_surface: EGLSurface,
    old_egl_context: EGLContext,
}

impl Drop for EGLBackedContext {
    #[inline]
    fn drop(&mut self) {
        if self.egl_context != egl::NO_CONTEXT && !thread::panicking() {
            panic!("Contexts must be destroyed explicitly with `destroy_context`!")
        }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        EGL_FUNCTIONS.with(|egl| unsafe {
            if self.egl_display != egl::NO_DISPLAY {
                egl.MakeCurrent(
                    self.egl_display,
                    self.old_egl_draw_surface,
                    self.old_egl_read_surface,
                    self.old_egl_context,
                );
            }
        })
    }
}

impl EGLBackedContext {
    pub(crate) unsafe fn new(
        egl_display: EGLDisplay,
        descriptor: &ContextDescriptor,
        share_with: Option<&EGLBackedContext>,
        gl_api: GLApi,
    ) -> Result<EGLBackedContext, Error> {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();

        // Create the context.
        let egl_context = create_context(
            egl_display,
            descriptor,
            share_with.map_or(egl::NO_CONTEXT, |ctx| ctx.egl_context),
            gl_api,
        )?;

        // Wrap and return it.
        let context = EGLBackedContext {
            egl_context,
            id: *next_context_id,
            framebuffer: Framebuffer::None,
            context_is_owned: true,
        };
        next_context_id.0 += 1;
        Ok(context)
    }

    pub(crate) unsafe fn from_native_context(native_context: NativeContext) -> EGLBackedContext {
        let mut next_context_id = CREATE_CONTEXT_MUTEX.lock().unwrap();
        let context = EGLBackedContext {
            egl_context: native_context.egl_context,
            id: *next_context_id,
            framebuffer: Framebuffer::External(ExternalEGLSurfaces {
                draw: native_context.egl_draw_surface,
                read: native_context.egl_read_surface,
            }),
            context_is_owned: false,
        };
        next_context_id.0 += 1;
        context
    }

    pub(crate) unsafe fn destroy(&mut self, egl_display: EGLDisplay) {
        EGL_FUNCTIONS.with(|egl| {
            egl.MakeCurrent(
                egl_display,
                egl::NO_SURFACE,
                egl::NO_SURFACE,
                egl::NO_CONTEXT,
            );

            if self.context_is_owned {
                let result = egl.DestroyContext(egl_display, self.egl_context);
                assert_ne!(result, egl::FALSE);
            }

            self.egl_context = egl::NO_CONTEXT;
        });
    }

    pub(crate) fn native_context(&self) -> NativeContext {
        let egl_surfaces = match self.framebuffer {
            Framebuffer::Surface(ref surface) => surface.egl_surfaces(),
            Framebuffer::External(ref surfaces) => (*surfaces).clone(),
            Framebuffer::None => ExternalEGLSurfaces::default(),
        };

        NativeContext {
            egl_context: self.egl_context,
            egl_draw_surface: egl_surfaces.draw,
            egl_read_surface: egl_surfaces.read,
        }
    }

    pub(crate) unsafe fn make_current(&self, egl_display: EGLDisplay) -> Result<(), Error> {
        let egl_surfaces = match self.framebuffer {
            Framebuffer::Surface(ref surface) => surface.egl_surfaces(),
            Framebuffer::External(ref surfaces) => (*surfaces).clone(),
            Framebuffer::None => ExternalEGLSurfaces::default(),
        };

        EGL_FUNCTIONS.with(|egl| {
            let result = egl.MakeCurrent(
                egl_display,
                egl_surfaces.draw,
                egl_surfaces.read,
                self.egl_context,
            );
            if result == egl::FALSE {
                let err = egl.GetError().to_windowing_api_error();
                return Err(Error::MakeCurrentFailed(err));
            }
            Ok(())
        })
    }

    #[inline]
    pub(crate) fn is_current(&self) -> bool {
        unsafe { EGL_FUNCTIONS.with(|egl| egl.GetCurrentContext() == self.egl_context) }
    }

    pub(crate) unsafe fn bind_surface(
        &mut self,
        egl_display: EGLDisplay,
        surface: EGLBackedSurface,
    ) -> Result<(), (Error, EGLBackedSurface)> {
        if self.id != surface.context_id {
            return Err((Error::IncompatibleSurface, surface));
        }

        match self.framebuffer {
            Framebuffer::None => self.framebuffer = Framebuffer::Surface(surface),
            Framebuffer::External(_) => return Err((Error::ExternalRenderTarget, surface)),
            Framebuffer::Surface(_) => return Err((Error::SurfaceAlreadyBound, surface)),
        }

        // If we're current, call `make_context_current()` again to switch to the new framebuffer.
        if self.is_current() {
            drop(self.make_current(egl_display))
        }

        Ok(())
    }

    pub(crate) unsafe fn unbind_surface(
        &mut self,
        gl: &Gl,
        egl_display: EGLDisplay,
    ) -> Result<Option<EGLBackedSurface>, Error> {
        match self.framebuffer {
            Framebuffer::None => return Ok(None),
            Framebuffer::Surface(_) => {}
            Framebuffer::External(_) => return Err(Error::ExternalRenderTarget),
        }

        let surface = match mem::replace(&mut self.framebuffer, Framebuffer::None) {
            Framebuffer::Surface(surface) => surface,
            Framebuffer::None | Framebuffer::External(_) => unreachable!(),
        };

        // If we're current, we stay current, but with no surface attached.
        surface.unbind(gl, egl_display, self.egl_context);

        Ok(Some(surface))
    }

    pub(crate) fn surface_info(&self) -> Result<Option<SurfaceInfo>, Error> {
        match self.framebuffer {
            Framebuffer::None => Ok(None),
            Framebuffer::External(_) => Err(Error::ExternalRenderTarget),
            Framebuffer::Surface(ref surface) => Ok(Some(surface.info())),
        }
    }
}

impl NativeContext {
    /// Returns the current EGL context and surfaces, if applicable.
    ///
    /// If there is no current EGL context, this returns a `NoCurrentContext` error.
    pub fn current() -> Result<NativeContext, Error> {
        EGL_FUNCTIONS.with(|egl| unsafe {
            let egl_context = egl.GetCurrentContext();
            if egl_context == egl::NO_CONTEXT {
                Err(Error::NoCurrentContext)
            } else {
                Ok(NativeContext {
                    egl_context,
                    egl_read_surface: egl.GetCurrentSurface(egl::READ as EGLint),
                    egl_draw_surface: egl.GetCurrentSurface(egl::DRAW as EGLint),
                })
            }
        })
    }
}

impl ContextDescriptor {
    pub(crate) unsafe fn new(
        egl_display: EGLDisplay,
        attributes: &ContextAttributes,
        extra_config_attributes: &[EGLint],
    ) -> Result<ContextDescriptor, Error> {
        let flags = attributes.flags;

        let alpha_size = if flags.contains(ContextAttributeFlags::ALPHA) {
            8
        } else {
            0
        };
        let depth_size = if flags.contains(ContextAttributeFlags::DEPTH) {
            24
        } else {
            0
        };
        let stencil_size = if flags.contains(ContextAttributeFlags::STENCIL) {
            8
        } else {
            0
        };

        let compatibility_profile = flags.contains(ContextAttributeFlags::COMPATIBILITY_PROFILE);

        // Mesa doesn't support the OpenGL compatibility profile post version 3.0. Take that into
        // account.
        if compatibility_profile
            && (attributes.version.major > 3
                || attributes.version.major == 3 && attributes.version.minor > 0)
        {
            return Err(Error::UnsupportedGLProfile);
        }

        // Create required config attributes.
        //
        // We check these separately because `eglChooseConfig` on its own might give us 32-bit
        // color when 24-bit color is requested, and that can break code.
        let required_config_attributes = [
            egl::RED_SIZE as EGLint,
            RGB_CHANNEL_BIT_DEPTH,
            egl::GREEN_SIZE as EGLint,
            RGB_CHANNEL_BIT_DEPTH,
            egl::BLUE_SIZE as EGLint,
            RGB_CHANNEL_BIT_DEPTH,
        ];

        // Create config attributes.
        let mut requested_config_attributes = required_config_attributes.to_vec();
        requested_config_attributes.extend_from_slice(&[
            egl::ALPHA_SIZE as EGLint,
            alpha_size,
            egl::DEPTH_SIZE as EGLint,
            depth_size,
            egl::STENCIL_SIZE as EGLint,
            stencil_size,
        ]);
        requested_config_attributes.extend_from_slice(extra_config_attributes);
        requested_config_attributes.extend_from_slice(&[egl::NONE as EGLint, 0, 0, 0]);

        EGL_FUNCTIONS.with(|egl| {
            // See how many applicable configs there are.
            let mut config_count = 0;
            let result = egl.ChooseConfig(
                egl_display,
                requested_config_attributes.as_ptr(),
                ptr::null_mut(),
                0,
                &mut config_count,
            );
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
            let result = egl.ChooseConfig(
                egl_display,
                requested_config_attributes.as_ptr(),
                configs.as_mut_ptr(),
                config_count,
                &mut real_config_count,
            );
            if result == egl::FALSE {
                let err = egl.GetError().to_windowing_api_error();
                return Err(Error::PixelFormatSelectionFailed(err));
            }

            // Sanitize configs.
            let egl_config = configs
                .into_iter()
                .filter(|&egl_config| {
                    required_config_attributes
                        .chunks(2)
                        .all(|pair| get_config_attr(egl_display, egl_config, pair[0]) == pair[1])
                })
                .next();
            let egl_config = match egl_config {
                None => return Err(Error::NoPixelFormatFound),
                Some(egl_config) => egl_config,
            };

            // Get the config ID and version.
            let egl_config_id = get_config_attr(egl_display, egl_config, egl::CONFIG_ID as EGLint);
            let gl_version = attributes.version;

            Ok(ContextDescriptor {
                egl_config_id,
                gl_version,
                compatibility_profile,
            })
        })
    }

    pub(crate) unsafe fn from_egl_context(
        gl: &Gl,
        egl_display: EGLDisplay,
        egl_context: EGLContext,
    ) -> ContextDescriptor {
        let egl_config_id = get_context_attr(egl_display, egl_context, egl::CONFIG_ID as EGLint);

        EGL_FUNCTIONS.with(|egl| {
            let _guard = CurrentContextGuard::new();
            egl.MakeCurrent(egl_display, egl::NO_SURFACE, egl::NO_SURFACE, egl_context);
            let gl_version = GLVersion::current(gl);
            let compatibility_profile = context::current_context_uses_compatibility_profile(gl);

            ContextDescriptor {
                egl_config_id,
                gl_version,
                compatibility_profile,
            }
        })
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn to_egl_config(&self, egl_display: EGLDisplay) -> EGLConfig {
        let config_attributes = [
            egl::CONFIG_ID as EGLint,
            self.egl_config_id,
            egl::NONE as EGLint,
            0,
            0,
            0,
        ];

        EGL_FUNCTIONS.with(|egl| {
            let (mut config, mut config_count) = (ptr::null(), 0);
            let result = egl.ChooseConfig(
                egl_display,
                config_attributes.as_ptr(),
                &mut config,
                1,
                &mut config_count,
            );
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

        attribute_flags.set(
            ContextAttributeFlags::COMPATIBILITY_PROFILE,
            self.compatibility_profile,
        );

        // Create appropriate context attributes.
        ContextAttributes {
            flags: attribute_flags,
            version: self.gl_version,
        }
    }
}

impl CurrentContextGuard {
    pub(crate) fn new() -> CurrentContextGuard {
        EGL_FUNCTIONS.with(|egl| unsafe {
            CurrentContextGuard {
                egl_display: egl.GetCurrentDisplay(),
                old_egl_draw_surface: egl.GetCurrentSurface(egl::DRAW as EGLint),
                old_egl_read_surface: egl.GetCurrentSurface(egl::READ as EGLint),
                old_egl_context: egl.GetCurrentContext(),
            }
        })
    }
}

pub(crate) unsafe fn create_context(
    egl_display: EGLDisplay,
    descriptor: &ContextDescriptor,
    share_with: EGLContext,
    gl_api: GLApi,
) -> Result<EGLContext, Error> {
    EGL_FUNCTIONS.with(|egl| {
        let ok = egl.BindAPI(match gl_api {
            GLApi::GL => egl::OPENGL_API,
            GLApi::GLES => egl::OPENGL_ES_API,
        });
        assert_ne!(ok, egl::FALSE);
    });

    let egl_config = egl_config_from_id(egl_display, descriptor.egl_config_id);

    let mut egl_context_attributes = vec![
        egl::CONTEXT_CLIENT_VERSION as EGLint,
        descriptor.gl_version.major as EGLint,
        EGL_CONTEXT_MINOR_VERSION_KHR as EGLint,
        descriptor.gl_version.minor as EGLint,
    ];

    // D3D11 ANGLE doesn't seem happy if EGL_CONTEXT_OPENGL_PROFILE_MASK is set
    // to be a core profile.
    if descriptor.compatibility_profile {
        egl_context_attributes.extend(&[
            EGL_CONTEXT_OPENGL_PROFILE_MASK as EGLint,
            EGL_CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT,
        ]);
    }

    // Include some extra zeroes to work around broken implementations.
    //
    // FIXME(pcwalton): Which implementations are those? (This is copied from Gecko.)
    egl_context_attributes.extend(&[egl::NONE as EGLint, 0, 0, 0]);

    EGL_FUNCTIONS.with(|egl| {
        let egl_context = egl.CreateContext(
            egl_display,
            egl_config,
            share_with,
            egl_context_attributes.as_ptr(),
        );
        if egl_context == egl::NO_CONTEXT {
            let err = egl.GetError();
            let err = err.to_windowing_api_error();
            return Err(Error::ContextCreationFailed(err));
        }

        Ok(egl_context)
    })
}

pub(crate) unsafe fn make_no_context_current(egl_display: EGLDisplay) -> Result<(), Error> {
    EGL_FUNCTIONS.with(|egl| {
        let result = egl.MakeCurrent(
            egl_display,
            egl::NO_SURFACE,
            egl::NO_SURFACE,
            egl::NO_CONTEXT,
        );
        if result == egl::FALSE {
            let err = egl.GetError().to_windowing_api_error();
            return Err(Error::MakeCurrentFailed(err));
        }
        Ok(())
    })
}

pub(crate) unsafe fn get_config_attr(
    egl_display: EGLDisplay,
    egl_config: EGLConfig,
    attr: EGLint,
) -> EGLint {
    EGL_FUNCTIONS.with(|egl| {
        let mut value = 0;
        let result = egl.GetConfigAttrib(egl_display, egl_config, attr, &mut value);
        assert_ne!(result, egl::FALSE);
        value
    })
}

pub(crate) unsafe fn get_context_attr(
    egl_display: EGLDisplay,
    egl_context: EGLContext,
    attr: EGLint,
) -> EGLint {
    EGL_FUNCTIONS.with(|egl| {
        let mut value = 0;
        let result = egl.QueryContext(egl_display, egl_context, attr, &mut value);
        assert_ne!(result, egl::FALSE);
        value
    })
}

pub(crate) unsafe fn egl_config_from_id(
    egl_display: EGLDisplay,
    egl_config_id: EGLint,
) -> EGLConfig {
    let config_attributes = [
        egl::CONFIG_ID as EGLint,
        egl_config_id,
        egl::NONE as EGLint,
        0,
        0,
        0,
    ];

    EGL_FUNCTIONS.with(|egl| {
        let (mut config, mut config_count) = (ptr::null(), 0);
        let result = egl.ChooseConfig(
            egl_display,
            config_attributes.as_ptr(),
            &mut config,
            1,
            &mut config_count,
        );
        assert_ne!(result, egl::FALSE);
        assert!(config_count > 0);
        config
    })
}

pub(crate) fn get_proc_address(symbol_name: &str) -> *const c_void {
    EGL_FUNCTIONS.with(|egl| unsafe {
        let symbol_name: CString = CString::new(symbol_name).unwrap();
        egl.GetProcAddress(symbol_name.as_ptr() as *const u8 as *const c_char) as *const c_void
    })
}

// Creates and returns a dummy pbuffer surface for the given context. This is used as the default
// framebuffer on some backends.
#[allow(dead_code)]
pub(crate) unsafe fn create_dummy_pbuffer(
    egl_display: EGLDisplay,
    egl_context: EGLContext,
) -> EGLSurface {
    let egl_config_id = get_context_attr(egl_display, egl_context, egl::CONFIG_ID as EGLint);
    let egl_config = egl_config_from_id(egl_display, egl_config_id);

    let pbuffer_attributes = [
        egl::WIDTH as EGLint,
        DUMMY_PBUFFER_SIZE,
        egl::HEIGHT as EGLint,
        DUMMY_PBUFFER_SIZE,
        egl::NONE as EGLint,
        0,
        0,
        0,
    ];

    EGL_FUNCTIONS.with(|egl| {
        let pbuffer =
            egl.CreatePbufferSurface(egl_display, egl_config, pbuffer_attributes.as_ptr());
        assert_ne!(pbuffer, egl::NO_SURFACE);
        pbuffer
    })
}
