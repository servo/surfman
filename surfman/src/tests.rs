// surfman/surfman/src/tests.rs
//
//! Unit tests.

use crate::gl::types::{GLenum, GLuint};
use crate::gl::{self, Gl};
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLVersion, SurfaceAccess};
use crate::{SurfaceType, WindowingApiError};
use super::connection::Connection;
use super::context::{Context, ContextDescriptor};
use super::device::{Adapter, Device};
use super::surface::Surface;

use euclid::default::Size2D;
use std::os::raw::c_void;
use std::sync::mpsc;
use std::thread;

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
    match connection.create_device(&adapter) {
        Ok(device) => {}
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    }
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
                Err(error) => {
                    panic!("Failed to create context ({:?}/{:?}): {:?}", version, flags, error)
                }
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
    let mut env = match BasicEnvironment::new() {
        None => return,
        Some(env) => env,
    };

    unsafe {
        // Check basic clear.
        env.gl.ClearColor(0.0, 1.0, 0.0, 1.0);
        env.gl.Clear(gl::COLOR_BUFFER_BIT);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Check that GL commands don't work after `make_no_context_current()`.
        //
        // The `glGetError()` calls are there to clear any errors.
        env.device.make_no_context_current().unwrap();
        env.gl.ClearColor(1.0, 0.0, 0.0, 1.0); env.gl.GetError();
        env.gl.Clear(gl::COLOR_BUFFER_BIT); env.gl.GetError();
        env.device.make_context_current(&env.context).unwrap();

        let framebuffer_object = env.device
                                    .context_surface_info(&env.context)
                                    .unwrap()
                                    .unwrap()
                                    .framebuffer_object;
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Make sure GL commands don't work when no surface is attached.
        //
        // The `glGetError()` calls are there to clear any errors.
        let green_surface = env.device
                               .unbind_surface_from_context(&mut env.context)
                               .unwrap()
                               .unwrap();
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
        env.gl.ClearColor(1.0, 0.0, 0.0, 1.0); env.gl.GetError();
        env.gl.Clear(gl::COLOR_BUFFER_BIT); env.gl.GetError();
        env.device.bind_surface_to_context(&mut env.context, green_surface).unwrap();
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Make sure GL commands go to the right surface.
        let green_surface = env.device
                               .unbind_surface_from_context(&mut env.context)
                               .unwrap()
                               .unwrap();
        let red_surface = make_surface(&mut env.device, &env.context);
        env.device.bind_surface_to_context(&mut env.context, red_surface).unwrap();
        let red_framebuffer_object = env.device
                                        .context_surface_info(&env.context)
                                        .unwrap()
                                        .unwrap()
                                        .framebuffer_object;
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, red_framebuffer_object);
        env.gl.ClearColor(1.0, 0.0, 0.0, 1.0);
        env.gl.Clear(gl::COLOR_BUFFER_BIT);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let mut red_surface = env.device
                                 .unbind_surface_from_context(&mut env.context)
                                 .unwrap()
                                 .unwrap();
        env.device.bind_surface_to_context(&mut env.context, green_surface).unwrap();
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Clean up.
        env.device.destroy_surface(&mut env.context, &mut red_surface).unwrap();
    }

    env.device.destroy_context(&mut env.context).unwrap();
}

#[test]
fn test_surface_texture_blit_framebuffer() {
    let mut env = match BasicEnvironment::new() {
        None => return,
        Some(env) => env,
    };

    unsafe {
        clear(&env.gl, &[0, 255, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        let green_surface = env.device
                               .unbind_surface_from_context(&mut env.context)
                               .unwrap()
                               .unwrap();
        let green_surface_texture = env.device
                                       .create_surface_texture(&mut env.context, green_surface)
                                       .unwrap();

        let main_surface = make_surface(&mut env.device, &env.context);
        env.device.bind_surface_to_context(&mut env.context, main_surface).unwrap();

        let main_framebuffer_object = env.device
                                         .context_surface_info(&env.context)
                                         .unwrap()
                                         .unwrap()
                                         .framebuffer_object;
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, main_framebuffer_object);
        clear(&env.gl, &[255, 0, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let mut green_framebuffer_object =
            make_fbo(&env.gl,
                     env.device.surface_gl_texture_target(),
                     env.device.surface_texture_object(&green_surface_texture));

        // Blit to main framebuffer.
        blit_fbo(&env.gl, main_framebuffer_object, green_framebuffer_object);
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, main_framebuffer_object); check_gl(&env.gl);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Clean up.
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, 0); check_gl(&env.gl);
        env.gl.DeleteFramebuffers(1, &mut green_framebuffer_object);

        let mut green_surface = env.device
                                   .destroy_surface_texture(&mut env.context,
                                                            green_surface_texture)
                                   .unwrap();
        env.device.destroy_surface(&mut env.context, &mut green_surface).unwrap();
        env.device.destroy_context(&mut env.context).unwrap();
    }
}

#[test]
fn test_cross_device_surface_texture_blit_framebuffer() {
    let mut env = match BasicEnvironment::new() {
        None => return,
        Some(env) => env,
    };

    let mut other_device = env.connection.create_device(&env.adapter).unwrap();

    unsafe {
        clear(&env.gl, &[255, 0, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let mut other_context = other_device.create_context(&env.context_descriptor).unwrap();
        let other_surface = make_surface(&mut other_device, &other_context);
        other_device.bind_surface_to_context(&mut other_context, other_surface).unwrap();
        other_device.make_context_current(&other_context).unwrap();
        bind_context_fbo(&env.gl, &other_device, &other_context);

        clear(&env.gl, &[0, 255, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        let green_surface = other_device.unbind_surface_from_context(&mut other_context)
                                        .unwrap()
                                        .unwrap();
        let green_surface_texture = env.device
                                       .create_surface_texture(&mut env.context, green_surface)
                                       .unwrap();

        env.device.make_context_current(&env.context).unwrap();
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let mut green_framebuffer_object =
            make_fbo(&env.gl,
                     env.device.surface_gl_texture_target(),
                     env.device.surface_texture_object(&green_surface_texture));

        // Blit to main framebuffer.
        blit_fbo(&env.gl, context_fbo(&env.device, &env.context), green_framebuffer_object);
        bind_context_fbo(&env.gl, &env.device, &env.context);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Clean up.
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, 0); check_gl(&env.gl);
        env.gl.DeleteFramebuffers(1, &mut green_framebuffer_object);

        let mut green_surface = env.device
                                   .destroy_surface_texture(&mut env.context,
                                                            green_surface_texture)
                                   .unwrap();
        other_device.destroy_surface(&mut other_context, &mut green_surface).unwrap();
        other_device.destroy_context(&mut other_context).unwrap();
        env.device.destroy_context(&mut env.context).unwrap();
    }
}

#[test]
fn test_cross_thread_surface_texture_blit_framebuffer() {
    let mut env = match BasicEnvironment::new() {
        None => return,
        Some(env) => env,
    };

    let (to_main_sender, to_main_receiver) = mpsc::channel();
    let (to_worker_sender, to_worker_receiver) = mpsc::channel();

    let other_connection = env.connection.clone();
    let other_adapter = env.adapter.clone();
    let other_context_descriptor = env.context_descriptor.clone();
    thread::spawn(move || {
        let mut device = other_connection.create_device(&other_adapter).unwrap();
        let mut context = device.create_context(&other_context_descriptor).unwrap();
        let gl = Gl::load_with(|symbol| device.get_proc_address(&context, symbol));

        let surface = make_surface(&mut device, &context);
        device.bind_surface_to_context(&mut context, surface).unwrap();
        device.make_context_current(&context).unwrap();
        bind_context_fbo(&gl, &device, &context);

        clear(&gl, &[0, 255, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&gl), [0, 255, 0, 255]);

        let surface = device.unbind_surface_from_context(&mut context).unwrap().unwrap();
        to_main_sender.send(surface).unwrap();

        let mut surface = to_worker_receiver.recv().unwrap();
        device.destroy_surface(&mut context, &mut surface).unwrap();
        device.destroy_context(&mut context).unwrap();
    });

    unsafe {
        clear(&env.gl, &[255, 0, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let green_surface = to_main_receiver.recv().unwrap();
        let green_surface_texture = env.device
                                       .create_surface_texture(&mut env.context, green_surface)
                                       .unwrap();

        env.device.make_context_current(&env.context).unwrap();
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let mut green_framebuffer_object =
            make_fbo(&env.gl,
                     env.device.surface_gl_texture_target(),
                     env.device.surface_texture_object(&green_surface_texture));

        // Blit to main framebuffer.
        blit_fbo(&env.gl, context_fbo(&env.device, &env.context), green_framebuffer_object);
        bind_context_fbo(&env.gl, &env.device, &env.context);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        // Clean up.
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, 0); check_gl(&env.gl);
        env.gl.DeleteFramebuffers(1, &mut green_framebuffer_object);

        let green_surface = env.device
                               .destroy_surface_texture(&mut env.context, green_surface_texture)
                               .unwrap();
        to_worker_sender.send(green_surface).unwrap();

        env.device.destroy_context(&mut env.context).unwrap();
    }
}

// Tests that surface textures are not upside-down.
#[test]
fn test_surface_texture_right_side_up() {
    let mut env = match BasicEnvironment::new() {
        None => return,
        Some(env) => env,
    };

    unsafe {
        clear(&env.gl, &[255, 0, 0, 255]);
        clear_bottom_row(&env.gl, &[0, 255, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);

        let subsurface = env.device
                            .unbind_surface_from_context(&mut env.context)
                            .unwrap()
                            .unwrap();
        let subsurface_texture = env.device
                                    .create_surface_texture(&mut env.context, subsurface)
                                    .unwrap();

        let main_surface = make_surface(&mut env.device, &env.context);
        env.device.bind_surface_to_context(&mut env.context, main_surface).unwrap();

        let main_framebuffer_object = env.device
                                         .context_surface_info(&env.context)
                                         .unwrap()
                                         .unwrap()
                                         .framebuffer_object;
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, main_framebuffer_object);
        clear(&env.gl, &[255, 0, 0, 255]);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        let mut subframebuffer_object =
            make_fbo(&env.gl,
                     env.device.surface_gl_texture_target(),
                     env.device.surface_texture_object(&subsurface_texture));

        // Blit to main framebuffer.
        blit_fbo(&env.gl, main_framebuffer_object, subframebuffer_object);
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, main_framebuffer_object); check_gl(&env.gl);
        assert_eq!(get_pixel_from_bottom_row(&env.gl), [0, 255, 0, 255]);
        assert_eq!(get_pixel_from_second_from_bottom_row(&env.gl), [255, 0, 0, 255]);

        // Clean up.
        env.gl.BindFramebuffer(gl::FRAMEBUFFER, 0); check_gl(&env.gl);
        env.gl.DeleteFramebuffers(1, &mut subframebuffer_object);

        let mut subsurface = env.device
                                .destroy_surface_texture(&mut env.context, subsurface_texture)
                                .unwrap();
        env.device.destroy_surface(&mut env.context, &mut subsurface).unwrap();
        env.device.destroy_context(&mut env.context).unwrap();
    }
}

fn bind_context_fbo(gl: &Gl, device: &Device, context: &Context) {
    unsafe {
        gl.BindFramebuffer(gl::FRAMEBUFFER, context_fbo(device, context)); check_gl(&gl);
    }
}

fn context_fbo(device: &Device, context: &Context) -> GLuint {
    device.context_surface_info(context).unwrap().unwrap().framebuffer_object
}

fn make_surface(device: &mut Device, context: &Context) -> Surface {
    device.create_surface(&context,
                          SurfaceAccess::GPUOnly,
                          SurfaceType::Generic { size: Size2D::new(640, 480) })
          .unwrap()
}

fn blit_fbo(gl: &Gl, dest_fbo: GLuint, src_fbo: GLuint) {
    unsafe {
        gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, dest_fbo); check_gl(gl);
        gl.BindFramebuffer(gl::READ_FRAMEBUFFER, src_fbo); check_gl(gl);
        gl.BlitFramebuffer(0,
                           0,
                           640,
                           480,
                           0,
                           0,
                           640,
                           480,
                           gl::COLOR_BUFFER_BIT,
                           gl::NEAREST); check_gl(gl);
    }
}

fn make_fbo(gl: &Gl, texture_target: GLenum, texture: GLuint) -> GLuint {
    unsafe {
        let mut framebuffer_object = 0;
        gl.GenFramebuffers(1, &mut framebuffer_object); check_gl(&gl);
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object); check_gl(&gl);
        gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                gl::COLOR_ATTACHMENT0,
                                texture_target,
                                texture,
                                0); check_gl(&gl);
        assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER), gl::FRAMEBUFFER_COMPLETE);
        framebuffer_object
    }
}

struct BasicEnvironment {
    connection: Connection,
    adapter: Adapter,
    device: Device,
    context_descriptor: ContextDescriptor,
    context: Context,
    gl: Gl,
}

impl BasicEnvironment {
    fn new() -> Option<BasicEnvironment> {
        let connection = Connection::new().unwrap();
        let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
        let mut device = match connection.create_device(&adapter) {
            Ok(device) => device,
            Err(Error::RequiredExtensionUnavailable) => {
                // Can't run these tests on this hardware.
                return None;
            }
            Err(err) => panic!("Failed to create device: {:?}", err),
        };

        let context_descriptor = device.create_context_descriptor(&ContextAttributes {
            version: GLVersion::new(3, 0),
            flags: ContextAttributeFlags::empty(),
        }).unwrap();

        let mut context = device.create_context(&context_descriptor).unwrap();
        let surface = make_surface(&mut device, &context);
        device.bind_surface_to_context(&mut context, surface).unwrap();
        device.make_context_current(&context).unwrap();

        let gl = Gl::load_with(|symbol| device.get_proc_address(&context, symbol));

        unsafe {
            let framebuffer_object = device.context_surface_info(&context)
                                           .unwrap()
                                           .unwrap()
                                           .framebuffer_object;
            gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl.Viewport(0, 0, 640, 480);
        }

        Some(BasicEnvironment {
            connection,
            adapter,
            device,
            context_descriptor,
            context,
            gl,
        })
    }
}

fn clear(gl: &Gl, color: &[u8; 4]) {
    unsafe {
        gl.ClearColor(color[0] as f32 / 255.0,
                      color[1] as f32 / 255.0,
                      color[2] as f32 / 255.0,
                      color[3] as f32 / 255.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
    }
}

fn clear_bottom_row(gl: &Gl, color: &[u8; 4]) {
    unsafe {
        gl.Scissor(0, 0, 640, 1);
        gl.Enable(gl::SCISSOR_TEST);
        gl.ClearColor(color[0] as f32 / 255.0,
                      color[1] as f32 / 255.0,
                      color[2] as f32 / 255.0,
                      color[3] as f32 / 255.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
        gl.Disable(gl::SCISSOR_TEST);
        gl.Scissor(0, 0, 640, 480);
    }
}

fn get_pixel_from_bottom_row(gl: &Gl) -> [u8; 4] {
    unsafe {
        let mut pixel: [u8; 4] = [0; 4];
        gl.ReadPixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, pixel.as_mut_ptr() as *mut c_void);
        pixel
    }
}

fn get_pixel_from_second_from_bottom_row(gl: &Gl) -> [u8; 4] {
    unsafe {
        let mut pixel: [u8; 4] = [0; 4];
        gl.ReadPixels(0, 1, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, pixel.as_mut_ptr() as *mut c_void);
        pixel
    }
}

fn check_gl(gl: &Gl) {
    unsafe {
        assert_eq!(gl.GetError(), gl::NO_ERROR);
    }
}
