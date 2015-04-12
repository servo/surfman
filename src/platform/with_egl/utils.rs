use egl;
use std::mem;
use geom::Size2D;

fn create_pbuffer_surface(config: EGLConfig, size: Size2D<i32>) -> Result<EGLSurface, &'static str> {
    let mut attrs = [
        egl::WIDTH, size.width as EGLInt,
        egl::HEIGHT, size.heigh as EGLInt,
        egl_end_workarounding_bugs!()
    ];

    let surface = unsafe { egl::CreatePBufferSurface(egl::Display(), config, attrs.as_mut_ptr()) };

    if surface == 0 {
        return Err("egl::CreatePBufferSurface");
    }

    Ok(surface)
}

fn create_pixel_buffer_backed_offscreen_context(size: Size2D<i32>) -> Result<GLContext, &'static str> {
    let mut attributes = [
        egl::SURFACE_TYPE, egl::PBUFFER_BIT,
        egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT,
        egl::RED_SIZE, 8,
        egl::GREEN_SIZE, 8,
        egl::BLUE_SIZE, 8,
        egl::ALPHA_SIZE, 0,
        egl_end_workarounding_bugs!()
    ];

    let config : EGLConfig = unsafe { mem::uninitialized() };
    let mut found_configs : EGLint = 0;

    if egl::ChooseConfig(egl::Display(), attributes.as_mut_ptr(), &mut config, 1, &mut found_configs) == 0 {
        return Err("egl::ChooseConfig");
    }

    if found_configs == 0 {
        return Err("No EGL config for pBuffer");
    }

    let surface = try!(create_pbuffer_surface(config, size));

    GLContext::new(None, true, surface, config)
}
