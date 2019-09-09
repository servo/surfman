use std::mem;
use euclid::default::Size2D;
use super::{NativeGLContext, NativeGLContextHandle};

use crate::GLVersion;
use crate::egl;
use crate::egl::types::{EGLNativeDisplayType, EGLDisplay, EGLConfig, EGLSurface, EGLint};
use sparkle::gl;

fn create_pbuffer_surface(display: EGLDisplay, config: EGLConfig, size: Size2D<i32>) -> Result<EGLSurface, &'static str> {
    let mut attrs = [
        egl::WIDTH as EGLint, size.width as EGLint,
        egl::HEIGHT as EGLint, size.height as EGLint,
        egl::NONE as EGLint, 0, 0, 0, // see mod.rs
    ];

    let surface = unsafe { egl::CreatePbufferSurface(display, config, &mut *attrs.as_mut_ptr()) };

    if surface == (egl::NO_SURFACE as EGLSurface) {
        return Err("egl::CreatePBufferSurface");
    }

    Ok(surface)
}

pub fn create_pixel_buffer_backed_offscreen_context(size: Size2D<i32>,
                                                    shared_with: Option<&NativeGLContextHandle>,
                                                    api_type: &gl::GlType,
                                                    api_version: GLVersion) -> Result<NativeGLContext, &'static str>  {
    let client_version = api_version.major_version();
    let renderable_type = match *api_type {
        gl::GlType::Gl => {
            egl::OPENGL_BIT
        },
        gl::GlType::Gles => {
            if client_version >= 3 {
                egl::OPENGL_ES3_BIT
            } else if client_version == 2 {
                egl::OPENGL_ES2_BIT
            } else {
                egl::OPENGL_ES_BIT
            }
        },
    };

    let attributes = [
        egl::SURFACE_TYPE as EGLint, egl::PBUFFER_BIT as EGLint,
        egl::RENDERABLE_TYPE as EGLint, renderable_type as EGLint,
        egl::RED_SIZE as EGLint, 8,
        egl::GREEN_SIZE as EGLint, 8,
        egl::BLUE_SIZE as EGLint, 8,
        egl::ALPHA_SIZE as EGLint, 0,
        egl::NONE as EGLint, 0, 0, 0, // see mod.rs
    ];

    let (shared_with, display) = match shared_with {
        Some(handle) => (Some(&handle.0), handle.1),
        None => {
            unsafe {
                let display = egl::GetDisplay(egl::DEFAULT_DISPLAY as EGLNativeDisplayType);

                if display == (egl::NO_DISPLAY as EGLDisplay) {
                    return Err("egl::GetDisplay");
                }

                // TODO: Ensure this is correct. It seems it's refcounted, but not atomically, so
                // we can't `Terminate` it on drop.
                //
                // It's the default display anyways so it is not a big problem.
                if egl::Initialize(display, 0 as *mut _, 0 as *mut _) == 0 {
                    return Err("egl::Initialize");
                }

                (None, display)
            }
        }
    };


    if display == (egl::NO_DISPLAY as EGLDisplay) {
        return Err("egl::GetDisplay");
    }

    let mut config : EGLConfig = unsafe { mem::uninitialized() };
    let mut found_configs : EGLint = 0;

    unsafe {
        if egl::ChooseConfig(display,
                             attributes.as_ptr(),
                             &mut config,
                             1,
                             &mut found_configs) == egl::FALSE as u32 {
            return Err("egl::ChooseConfig");
        }
    }

    if found_configs == 0 {
        return Err("No EGL config for pBuffer");
    }

    let surface = create_pbuffer_surface(display, config, size)?;

    NativeGLContext::new(shared_with, display, surface, config, client_version)
}
