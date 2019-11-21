// surfman/surfman/src/tests.rs
//
//! Unit tests.

use crate::gl::{self, Gl};
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLVersion, SurfaceAccess};
use crate::{SurfaceType, WindowingApiError};
use super::connection::Connection;
use super::device::Adapter;

use euclid::default::Size2D;
use std::os::raw::c_void;

static GL_VERSIONS: [GLVersion; 6] = [
    GLVersion { major: 2, minor: 0 },
    GLVersion { major: 3, minor: 0 },
    GLVersion { major: 3, minor: 1 },
    GLVersion { major: 3, minor: 2 },
    GLVersion { major: 4, minor: 0 },
    GLVersion { major: 4, minor: 1 },
];

static GL_ES_VERSIONS: [GLVersion; 4] = [
    GLVersion { major: 2, minor: 0 },
    GLVersion { major: 3, minor: 0 },
    GLVersion { major: 3, minor: 1 },
    GLVersion { major: 3, minor: 2 },
];

#[test]
fn test_adapter_creation() {
    let connection = Connection::new().unwrap();
    connection.create_hardware_adapter().unwrap();
    connection.create_low_power_adapter().unwrap();
    connection.create_software_adapter().unwrap();
}

#[test]
fn test_device_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };
}

#[test]
fn test_device_accessors() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().unwrap();
    let device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };
    drop(device.connection());
    drop(device.adapter());
    drop(device.gl_api());
}

// Tests that all combinations of flags result in the creation of valid context descriptors and
// contexts.
#[test]
fn test_context_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let mut device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };

    let versions = match device.gl_api() {
        GLApi::GL => &GL_VERSIONS[..],
        GLApi::GLES => &GL_ES_VERSIONS[..],
    };

    for &version in versions {
        for flag_bits in 0..(ContextAttributeFlags::all().bits() + 1) {
            let flags = ContextAttributeFlags::from_bits_truncate(flag_bits);
            let attributes = ContextAttributes { version, flags };
            println!("Creating context with attributes: {:?}", attributes);
            let descriptor = match device.create_context_descriptor(&attributes) {
                Ok(descriptor) => descriptor,
                Err(Error::UnsupportedGLProfile) | Err(Error::UnsupportedGLVersion) => {
                    // Nothing we can do about this. Go on to the next one.
                    continue
                }
                Err(err) => panic!("Context descriptor creation failed: {:?}", err),
            };

            match device.create_context(&descriptor) {
                Ok(mut context) => {
                    // Verify that the attributes round-trip.
                    let actual_descriptor = device.context_descriptor(&context);
                    let actual_attributes =
                        device.context_descriptor_attributes(&actual_descriptor);
                    if !actual_attributes.flags.contains(attributes.flags) {
                        device.destroy_context(&mut context).unwrap();
                        panic!("Expected at least attribute flags {:?} but got {:?}",
                               attributes.flags,
                               actual_attributes.flags);
                    }
                    if actual_attributes.version.major < attributes.version.major ||
                            (actual_attributes.version.major == attributes.version.major &&
                             actual_attributes.version.minor < attributes.version.minor) {
                        device.destroy_context(&mut context).unwrap();
                        panic!("Expected at least GL version {:?} but got version {:?}",
                               attributes,
                               actual_attributes);
                    }

                    device.destroy_context(&mut context).unwrap();
                }
                Err(Error::ContextCreationFailed(WindowingApiError::BadPixelFormat)) => {
                    // This is OK, as it just means the GL implementation didn't support the
                    // requested GL version.
                }
                Err(error) => panic!("Failed to create context: {:?}", error),
            }
        }
    }
}

// Tests that generic surfaces can be created.
#[test]
fn test_generic_surface_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let mut device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };

    let descriptor = device.create_context_descriptor(&ContextAttributes {
        version: GLVersion::new(3, 0),
        flags: ContextAttributeFlags::empty(),
    }).unwrap();

    let mut context = device.create_context(&descriptor).unwrap();
    let context_id = device.context_id(&context);

    let surfaces: Vec<_> = [
        SurfaceAccess::GPUOnly,
        SurfaceAccess::GPUCPU,
        SurfaceAccess::GPUCPUWriteCombined,
    ].iter().map(|&access| {
        let surface = device.create_surface(&context, access, SurfaceType::Generic {
            size: Size2D::new(640, 480),
        }).unwrap();
        let info = device.surface_info(&surface);
        assert_eq!(info.size, Size2D::new(640, 480));
        assert_eq!(info.context_id, context_id);
        surface
    }).collect();

    // Make sure all IDs for extant surfaces are distinct.
    for (surface_index, surface) in surfaces.iter().enumerate() {
        for (other_surface_index, other_surface) in surfaces.iter().enumerate() {
            if surface_index != other_surface_index {
                assert_ne!(device.surface_info(surface).id, device.surface_info(other_surface).id);
            }
        }
    }

    for mut surface in surfaces.into_iter() {
        device.destroy_surface(&mut context, &mut surface).unwrap();
    }

    device.destroy_context(&mut context).unwrap();
}

// Tests that basic GL commands work.
#[test]
fn test_gl() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let mut device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };

    let descriptor = device.create_context_descriptor(&ContextAttributes {
        version: GLVersion::new(3, 0),
        flags: ContextAttributeFlags::empty(),
    }).unwrap();

    let mut context = device.create_context(&descriptor).unwrap();

    let surface = device.create_surface(&context, SurfaceAccess::GPUOnly, SurfaceType::Generic {
        size: Size2D::new(640, 480),
    }).unwrap();
    device.bind_surface_to_context(&mut context, surface).unwrap();
    device.make_context_current(&context).unwrap();

    unsafe {
        let gl = Gl::load_with(|symbol| device.get_proc_address(&context, symbol));
        let framebuffer_object = device.context_surface_info(&context)
                                       .unwrap()
                                       .unwrap()
                                       .framebuffer_object;
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        gl.Viewport(0, 0, 640, 480);

        // Check basic clear.
        gl.ClearColor(0.0, 1.0, 0.0, 1.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
        assert_eq!(get_pixel(&gl), [0, 255, 0, 255]);

        // Check that GL commands don't work after `make_no_context_current()`.
        //
        // The `glGetError()` calls are there to clear any errors.
        device.make_no_context_current().unwrap();
        gl.ClearColor(1.0, 0.0, 0.0, 1.0);
        gl.GetError();
        gl.Clear(gl::COLOR_BUFFER_BIT);
        gl.GetError();
        device.make_context_current(&context).unwrap();
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        assert_eq!(get_pixel(&gl), [0, 255, 0, 255]);

        // Make sure GL commands don't work when no surface is attached.
        //
        // The `glGetError()` calls are there to clear any errors.
        let green_surface = device.unbind_surface_from_context(&mut context).unwrap().unwrap();
        gl.ClearColor(1.0, 0.0, 0.0, 1.0);
        gl.GetError();
        gl.Clear(gl::COLOR_BUFFER_BIT);
        gl.GetError();
        device.bind_surface_to_context(&mut context, green_surface).unwrap();
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        assert_eq!(get_pixel(&gl), [0, 255, 0, 255]);

        // Make sure GL commands go to the right surface.
        let green_surface = device.unbind_surface_from_context(&mut context).unwrap().unwrap();
        let red_surface = device.create_surface(&context,
                                                SurfaceAccess::GPUOnly,
                                                SurfaceType::Generic {
            size: Size2D::new(640, 480),
        }).unwrap();
        device.bind_surface_to_context(&mut context, red_surface).unwrap();
        let red_framebuffer_object = device.context_surface_info(&context)
                                           .unwrap()
                                           .unwrap()
                                           .framebuffer_object;
        gl.BindFramebuffer(gl::FRAMEBUFFER, red_framebuffer_object);
        gl.ClearColor(1.0, 0.0, 0.0, 1.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
        let mut red_surface = device.unbind_surface_from_context(&mut context).unwrap().unwrap();
        device.bind_surface_to_context(&mut context, green_surface).unwrap();
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        assert_eq!(get_pixel(&gl), [0, 255, 0, 255]);

        // Clean up.
        device.destroy_surface(&mut context, &mut red_surface).unwrap();
    }

    device.destroy_context(&mut context).unwrap();
}

#[test]
fn test_surface_texture_blit_framebuffer() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let mut device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };

    let descriptor = device.create_context_descriptor(&ContextAttributes {
        version: GLVersion::new(3, 0),
        flags: ContextAttributeFlags::empty(),
    }).unwrap();

    let mut context = device.create_context(&descriptor).unwrap();

    let size = Size2D::new(640, 480);
    let green_surface = device.create_surface(&context,
                                              SurfaceAccess::GPUOnly,
                                              SurfaceType::Generic { size }).unwrap();
    device.bind_surface_to_context(&mut context, green_surface).unwrap();
    device.make_context_current(&context).unwrap();

    unsafe {
        let gl = Gl::load_with(|symbol| device.get_proc_address(&context, symbol));
        let green_framebuffer_object = device.context_surface_info(&context)
                                             .unwrap()
                                             .unwrap()
                                             .framebuffer_object;
        gl.BindFramebuffer(gl::FRAMEBUFFER, green_framebuffer_object);
        gl.Viewport(0, 0, 640, 480);
        gl.ClearColor(0.0, 1.0, 0.0, 1.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
        assert_eq!(get_pixel(&gl), [0, 255, 0, 255]);

        let green_surface = device.unbind_surface_from_context(&mut context).unwrap().unwrap();
        let green_surface_texture = device.create_surface_texture(&mut context, green_surface)
                                          .unwrap();

        let main_surface = device.create_surface(&context,
                                                 SurfaceAccess::GPUOnly,
                                                 SurfaceType::Generic { size }).unwrap();
        device.bind_surface_to_context(&mut context, main_surface).unwrap();
        device.make_context_current(&context).unwrap(); // FIXME(pcwalton): Shouldn't be necessary.
        let main_framebuffer_object = device.context_surface_info(&context)
                                            .unwrap()
                                            .unwrap()
                                            .framebuffer_object;
        gl.BindFramebuffer(gl::FRAMEBUFFER, main_framebuffer_object);
        gl.Viewport(0, 0, 640, 480);
        gl.ClearColor(1.0, 0.0, 0.0, 1.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
        assert_eq!(get_pixel(&gl), [255, 0, 0, 255]);

        let mut green_framebuffer_object = 0;
        gl.GenFramebuffers(1, &mut green_framebuffer_object); check_gl(&gl);
        gl.BindFramebuffer(gl::FRAMEBUFFER, green_framebuffer_object); check_gl(&gl);
        gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                gl::COLOR_ATTACHMENT0,
                                device.surface_gl_texture_target(),
                                device.surface_texture_object(&green_surface_texture),
                                0); check_gl(&gl);
        assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER), gl::FRAMEBUFFER_COMPLETE);

        // Blit to main framebuffer.
        gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, main_framebuffer_object); check_gl(&gl);
        gl.BindFramebuffer(gl::READ_FRAMEBUFFER, green_framebuffer_object); check_gl(&gl);
        gl.BlitFramebuffer(0,
                           0,
                           640,
                           480,
                           0,
                           0,
                           640,
                           480,
                           gl::COLOR_BUFFER_BIT,
                           gl::NEAREST); check_gl(&gl);
        gl.BindFramebuffer(gl::FRAMEBUFFER, main_framebuffer_object); check_gl(&gl);
        assert_eq!(get_pixel(&gl), [0, 255, 0, 255]);

        // Clean up.
        gl.BindFramebuffer(gl::FRAMEBUFFER, 0); check_gl(&gl);
        gl.DeleteFramebuffers(1, &mut green_framebuffer_object);

        let mut green_surface = device.destroy_surface_texture(&mut context, green_surface_texture)
                                      .unwrap();
        device.destroy_surface(&mut context, &mut green_surface).unwrap();
        device.destroy_context(&mut context).unwrap();
    }
}

fn get_pixel(gl: &Gl) -> [u8; 4] {
    unsafe {
        let mut pixel: [u8; 4] = [0; 4];
        gl.ReadPixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, pixel.as_mut_ptr() as *mut c_void);
        pixel
    }
}

fn check_gl(gl: &Gl) {
    unsafe {
        assert_eq!(gl.GetError(), gl::NO_ERROR);
    }
}
