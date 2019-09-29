// surfman/examples/threads.rs
//
// This example demonstrates how to create a multithreaded OpenGL application using `surfman`.

use crate::common::{Buffer, Program, Shader, ShaderKind, ck};

use euclid::default::{Point2D, Rect, Size2D, Vector2D};
use gl::types::{GLchar, GLenum, GLint, GLuint, GLvoid};
use sdl2::event::Event;
use sdl2::hint;
use sdl2::keyboard::Keycode;
use sdl2::video::{GLProfile, SwapInterval};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use surfman::{Adapter, ContextDescriptor, Device, GLApi, Surface};

mod common;

const WINDOW_WIDTH:  i32 = 800;
const WINDOW_HEIGHT: i32 = 600;

const SUBSCREEN_WIDTH:  i32 = 256;
const SUBSCREEN_HEIGHT: i32 = 256;

const BALL_WIDTH:  i32 = 192;
const BALL_HEIGHT: i32 = 192;

const CHECK_SIZE: f32 = 16.0;

const INITIAL_VELOCITY_X: f32 = 1.5;
const INITIAL_VELOCITY_Y: f32 = 0.0;
const GRAVITY: f32 = -0.2;
const INITIAL_ROTATION_X: f32 = 0.2;
const INITIAL_ROTATION_Y: f32 = 0.6;
const INITIAL_ROTATION_Z: f32 = 0.2;
const ROTATION_SPEED_X: f32 = 0.03;
const ROTATION_SPEED_Y: f32 = 0.01;
const ROTATION_SPEED_Z: f32 = 0.05;

const SPHERE_RADIUS: f32 = 96.0;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 0, 1, 1, 1 ];

static BLIT_TRANSFORM: [f32; 4] = [
    SUBSCREEN_WIDTH as f32 / WINDOW_WIDTH as f32 * 2.0, 0.0,
    0.0, SUBSCREEN_HEIGHT as f32 / WINDOW_HEIGHT as f32 * 2.0,
];

static CHECK_TRANSFORM: [f32; 4] = [
    SUBSCREEN_WIDTH as f32 / CHECK_SIZE as f32, 0.0,
    0.0, SUBSCREEN_HEIGHT as f32 / CHECK_SIZE as f32,
];

static IDENTITY_TRANSFORM:      [f32; 4] = [1.0, 0.0, 0.0, 1.0];
static ZERO_TRANSLATION:        [f32; 2] = [0.0, 0.0];

static NDC_TRANSFORM:   [f32; 4] = [2.0, 0.0, 0.0, 2.0];
static NDC_TRANSLATION: [f32; 2] = [-1.0, -1.0];

static CHECK_COLOR_A: [f32; 4] = [0.8, 0.0, 0.0, 1.0];
static CHECK_COLOR_B: [f32; 4] = [0.9, 0.9, 0.9, 1.0];

static CAMERA_POSITION: [f32; 3] = [400.0, 300.0, -1000.0];
static LIGHT_POSITION:  [f32; 3] = [600.0, 450.0, -500.0];

fn main() {
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let gl_api = Device::gl_api();
    if gl_api == GLApi::GLES {
        hint::set("SDL_OPENGL_ES_DRIVER", "1");
    }
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context.
    let gl_attributes = video.gl_attr();
    if gl_api == GLApi::GLES {
        gl_attributes.set_context_profile(GLProfile::GLES);
        gl_attributes.set_context_version(3, 0);
    } else {
        gl_attributes.set_context_profile(GLProfile::Core);
        gl_attributes.set_context_version(3, 3);
    }

    // Open a window.
    let window = video.window("Multithreaded example", WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32)
                      .opengl()
                      .build()
                      .unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    // Create the GL context in SDL, and make it current.
    let gl_context = window.gl_create_context().unwrap();
    window.gl_make_current(&gl_context).unwrap();

    // Try to enable vsync, but ignore the error if we can't.
    drop(video.gl_set_swap_interval(SwapInterval::VSync));

    // Create `surfman` objects corresponding to that SDL context.
    let (device, mut context) = unsafe {
        Device::from_current_context().unwrap()
    };
    gl::load_with(|symbol_name| device.get_proc_address(&context, symbol_name));
    let adapter = device.adapter();
    let context_descriptor = device.context_descriptor(&context);

    // Set up communication channels, and spawn our worker thread.
    let (worker_to_main_sender, main_from_worker_receiver) = mpsc::channel();
    let (main_to_worker_sender, worker_from_main_receiver) = mpsc::channel();
    thread::spawn(move || {
        worker_thread(adapter,
                      context_descriptor,
                      worker_to_main_sender,
                      worker_from_main_receiver)
    });

    // Set up GL objects and state.
    let grid_vertex_array = GridVertexArray::new(device.surface_gl_texture_target());
    let blit_vertex_array = BlitVertexArray::new(device.surface_gl_texture_target());

    // Fetch our initial surface.
    let NewFrame { mut surface, origin: _ } = main_from_worker_receiver.recv().unwrap();
    let mut origin;
    let mut texture = device.create_surface_texture(&mut context, surface).unwrap();

    // Enter main render loop.
    loop {
        // Send back our old surface, and fetch a new one.
        surface = device.destroy_surface_texture(&mut context, texture).unwrap();
        main_to_worker_sender.send(surface).unwrap();
        let NewFrame {
            surface: new_surface,
            origin: new_origin,
        } = main_from_worker_receiver.recv().unwrap();
        surface = new_surface;
        origin = new_origin;
        texture = device.create_surface_texture(&mut context, surface).unwrap();

        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0); ck();
            gl::Clear(gl::COLOR_BUFFER_BIT); ck();

            // Draw gridlines.
            gl::BindVertexArray(grid_vertex_array.object); ck();
            gl::UseProgram(grid_vertex_array.grid_program.program.object); ck();
            gl::UniformMatrix2fv(grid_vertex_array.grid_program.transform_uniform,
                                 1,
                                 gl::FALSE,
                                 NDC_TRANSFORM.as_ptr());
            gl::Uniform2fv(grid_vertex_array.grid_program.translation_uniform,
                           1,
                           NDC_TRANSLATION.as_ptr());
            gl::UniformMatrix2fv(grid_vertex_array.grid_program.tex_transform_uniform,
                                 1,
                                 gl::FALSE,
                                 CHECK_TRANSFORM.as_ptr());
            gl::Uniform2fv(grid_vertex_array.grid_program.tex_translation_uniform,
                           1,
                           ZERO_TRANSLATION.as_ptr());
            gl::Uniform4fv(grid_vertex_array.grid_program.gridline_color_uniform,
                           1,
                           [1.0, 1.0, 1.0, 1.0].as_ptr());
            gl::Uniform4fv(grid_vertex_array.grid_program.bg_color_uniform,
                           1,
                           [0.0, 0.0, 0.0, 1.0].as_ptr());
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4); ck();

            // Draw subscreen.
            let subscreen_translation = Point2D::new(origin.x / WINDOW_WIDTH as f32 * 2.0 - 1.0,
                                                     origin.y / WINDOW_HEIGHT as f32 * 2.0 - 1.0);
            gl::BindVertexArray(blit_vertex_array.object); ck();
            gl::UseProgram(blit_vertex_array.blit_program.program.object); ck();
            gl::UniformMatrix2fv(blit_vertex_array.blit_program.transform_uniform,
                                 1,
                                 gl::FALSE,
                                 BLIT_TRANSFORM.as_ptr());
            gl::Uniform2fv(blit_vertex_array.blit_program.translation_uniform,
                           1,
                           [subscreen_translation.x, subscreen_translation.y].as_ptr());
            gl::UniformMatrix2fv(blit_vertex_array.blit_program.tex_transform_uniform,
                                 1,
                                 gl::FALSE,
                                 IDENTITY_TRANSFORM.as_ptr());
            gl::Uniform2fv(blit_vertex_array.blit_program.tex_translation_uniform,
                           1,
                           ZERO_TRANSLATION.as_ptr());
            gl::ActiveTexture(gl::TEXTURE0); ck();
            gl::BindTexture(device.surface_gl_texture_target(), texture.gl_texture()); ck();
            gl::Uniform1i(blit_vertex_array.blit_program.source_uniform, 0); ck();
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4); ck();
            gl::Disable(gl::BLEND);
        }

        window.gl_swap_window();

        loop {
            match event_pump.poll_event() {
                Some(Event::Quit {..}) |
                Some(Event::KeyDown { keycode: Some(Keycode::Escape), .. }) => return,
                None => break,
                _ => {}
            }
        }
    }
}

fn worker_thread(adapter: Adapter,
                 context_descriptor: ContextDescriptor,
                 worker_to_main_sender: Sender<NewFrame>,
                 worker_from_main_receiver: Receiver<Surface>) {
    // Open the device, create a context, and make it current.
    let size = Size2D::new(SUBSCREEN_WIDTH, SUBSCREEN_HEIGHT);
    let mut device = Device::new(&adapter).unwrap();
    let mut context = device.create_context(&context_descriptor, &size).unwrap();
    device.make_context_current(&context).unwrap();

    // Set up GL objects and state.
    let vertex_array = CheckVertexArray::new(device.surface_gl_texture_target());

    // Initialize our origin and size.
    let ball_origin = Point2D::new(WINDOW_WIDTH as f32 * 0.5 - BALL_WIDTH as f32 * 0.5,
                                   WINDOW_HEIGHT as f32 * 0.65 - BALL_HEIGHT as f32 * 0.5);
    let ball_size = Size2D::new(BALL_WIDTH as f32, BALL_HEIGHT as f32);
    let mut ball_rect = Rect::new(ball_origin, ball_size);
    let mut ball_velocity = Vector2D::new(INITIAL_VELOCITY_X, INITIAL_VELOCITY_Y);
    let subscreen_offset = (Point2D::new(SUBSCREEN_WIDTH as f32, SUBSCREEN_HEIGHT as f32) -
                            Point2D::new(BALL_WIDTH as f32, BALL_HEIGHT as f32)) * 0.5;

    // Initialize our rotation.
    let mut theta_x = INITIAL_ROTATION_X;
    let mut theta_y = INITIAL_ROTATION_Y;
    let mut theta_z = INITIAL_ROTATION_Z;

    // Send an initial surface back to the main thread.
    let surface = device.create_surface(&context, &size).unwrap();
    worker_to_main_sender.send(NewFrame {
        surface,
        origin: ball_rect.origin - subscreen_offset,
    }).unwrap();

    loop {
        // Render to the surface.
        unsafe {
            let framebuffer_object = device.context_surface_framebuffer_object(&context).unwrap();
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl::Viewport(0, 0, size.width, size.height);

            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::BindVertexArray(vertex_array.object); ck();
            gl::UseProgram(vertex_array.check_program.program.object); ck();
            gl::UniformMatrix2fv(vertex_array.check_program.transform_uniform,
                                 1,
                                 gl::FALSE,
                                 NDC_TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.check_program.translation_uniform,
                           1,
                           NDC_TRANSLATION.as_ptr());
            gl::UniformMatrix2fv(vertex_array.check_program.tex_transform_uniform,
                                 1,
                                 gl::FALSE,
                                 NDC_TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.check_program.tex_translation_uniform,
                           1,
                           NDC_TRANSLATION.as_ptr());
            gl::Uniform3fv(vertex_array.check_program.rotation_uniform,
                           1,
                           [theta_x, theta_y, theta_z].as_ptr());
            gl::Uniform4fv(vertex_array.check_program.color_a_uniform,
                           1,
                           CHECK_COLOR_A.as_ptr());
            gl::Uniform4fv(vertex_array.check_program.color_b_uniform,
                           1,
                           CHECK_COLOR_B.as_ptr());
            gl::Uniform2f(vertex_array.check_program.viewport_origin_uniform,
                          ball_rect.origin.x - subscreen_offset.x,
                          ball_rect.origin.y - subscreen_offset.y);
            gl::Uniform1f(vertex_array.check_program.radius_uniform, SPHERE_RADIUS);
            gl::Uniform3fv(vertex_array.check_program.camera_position_uniform,
                           1,
                           CAMERA_POSITION.as_ptr());
            gl::Uniform3fv(vertex_array.check_program.light_position_uniform,
                           1,
                           LIGHT_POSITION.as_ptr());
            gl::Uniform2f(vertex_array.check_program.sphere_position_uniform,
                          lerp(ball_rect.origin.x, ball_rect.max_x(), 0.5),
                          lerp(ball_rect.origin.y, ball_rect.max_y(), 0.5));
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4); ck();
        }

        let new_surface = worker_from_main_receiver.recv().unwrap();
        let old_surface = device.replace_context_surface(&mut context, new_surface).unwrap();
        worker_to_main_sender.send(NewFrame {
            surface: old_surface,
            origin: ball_rect.origin - subscreen_offset,
        }).unwrap();

        // Advance ball.
        ball_velocity += Vector2D::new(0.0, GRAVITY);
        ball_rect = ball_rect.translate(ball_velocity);

        // Bounce off edges.
        if ball_rect.origin.y <= 0.0 {
            ball_rect.origin.y = 0.0;
            ball_velocity.y = f32::abs(ball_velocity.y);
        }
        if ball_rect.origin.x <= 0.0 {
            ball_rect.origin.x = 0.0;
            ball_velocity.x = f32::abs(ball_velocity.x);
        }
        if ball_rect.max_x() >= WINDOW_WIDTH as f32 {
            ball_rect.origin.x = (WINDOW_WIDTH - BALL_WIDTH) as f32;
            ball_velocity.x = -f32::abs(ball_velocity.x);
        }

        // Rotate.
        theta_x += ROTATION_SPEED_X;
        theta_y += ROTATION_SPEED_Y;
        theta_z += ROTATION_SPEED_Z;
    }
}

struct NewFrame {
    surface: Surface,
    origin: Point2D<f32>,
}

struct BlitVertexArray {
    object: GLuint,
    blit_program: BlitProgram,
    #[allow(dead_code)]
    position_buffer: Buffer,
}

impl BlitVertexArray {
    fn new(gl_texture_target: GLenum) -> BlitVertexArray {
        let blit_program = BlitProgram::new(gl_texture_target);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array); ck();
            gl::BindVertexArray(vertex_array); ck();

            let position_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, position_buffer.object); ck();
            gl::VertexAttribPointer(blit_program.position_attribute as GLuint,
                                    2,
                                    gl::UNSIGNED_BYTE,
                                    gl::FALSE,
                                    2,
                                    0 as *const GLvoid); ck();
            gl::EnableVertexAttribArray(blit_program.position_attribute as GLuint); ck();

            BlitVertexArray { object: vertex_array, blit_program, position_buffer }
        }
    }
}

struct GridVertexArray {
    object: GLuint,
    grid_program: GridProgram,
    #[allow(dead_code)]
    position_buffer: Buffer,
}

impl GridVertexArray {
    fn new(gl_texture_target: GLenum) -> GridVertexArray {
        let grid_program = GridProgram::new(gl_texture_target);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array); ck();
            gl::BindVertexArray(vertex_array); ck();

            let position_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, position_buffer.object); ck();
            gl::VertexAttribPointer(grid_program.position_attribute as GLuint,
                                    2,
                                    gl::UNSIGNED_BYTE,
                                    gl::FALSE,
                                    2,
                                    0 as *const GLvoid); ck();
            gl::EnableVertexAttribArray(grid_program.position_attribute as GLuint); ck();

            GridVertexArray { object: vertex_array, grid_program, position_buffer }
        }
    }
}

struct CheckVertexArray {
    object: GLuint,
    check_program: CheckProgram,
    #[allow(dead_code)]
    position_buffer: Buffer,
}

impl CheckVertexArray {
    fn new(gl_texture_target: GLenum) -> CheckVertexArray {
        let check_program = CheckProgram::new(gl_texture_target);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array); ck();
            gl::BindVertexArray(vertex_array); ck();

            let position_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, position_buffer.object); ck();
            gl::VertexAttribPointer(check_program.position_attribute as GLuint,
                                    2,
                                    gl::UNSIGNED_BYTE,
                                    gl::FALSE,
                                    2,
                                    0 as *const GLvoid); ck();
            gl::EnableVertexAttribArray(check_program.position_attribute as GLuint); ck();

            CheckVertexArray { object: vertex_array, check_program, position_buffer }
        }
    }
}

struct BlitProgram {
    program: Program,
    position_attribute: GLint,
    transform_uniform: GLint,
    translation_uniform: GLint,
    tex_transform_uniform: GLint,
    tex_translation_uniform: GLint,
    source_uniform: GLint,
}

impl BlitProgram {
    fn new(gl_texture_target: GLenum) -> BlitProgram {
        let vertex_shader = Shader::new("quad", ShaderKind::Vertex, gl_texture_target);
        let fragment_shader = Shader::new("blit", ShaderKind::Fragment, gl_texture_target);
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object,
                                      b"aPosition\0".as_ptr() as *const GLchar); ck();
            let transform_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTransform\0".as_ptr() as *const GLchar); ck();
            let translation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTranslation\0".as_ptr() as *const GLchar); ck();
            let tex_transform_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTexTransform\0".as_ptr() as *const GLchar); ck();
            let tex_translation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTexTranslation\0".as_ptr() as *const GLchar); ck();
            let source_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uSource\0".as_ptr() as *const GLchar); ck();
            BlitProgram {
                program,
                position_attribute,
                transform_uniform,
                translation_uniform,
                tex_transform_uniform,
                tex_translation_uniform,
                source_uniform,
            }
        }
    }
}

struct GridProgram {
    program: Program,
    position_attribute: GLint,
    transform_uniform: GLint,
    translation_uniform: GLint,
    tex_transform_uniform: GLint,
    tex_translation_uniform: GLint,
    gridline_color_uniform: GLint,
    bg_color_uniform: GLint,
}

impl GridProgram {
    fn new(gl_texture_target: GLenum) -> GridProgram {
        let vertex_shader = Shader::new("quad", ShaderKind::Vertex, gl_texture_target);
        let fragment_shader = Shader::new("grid", ShaderKind::Fragment, gl_texture_target);
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object,
                                      b"aPosition\0".as_ptr() as *const GLchar); ck();
            let transform_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTransform\0".as_ptr() as *const GLchar); ck();
            let translation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTranslation\0".as_ptr() as *const GLchar); ck();
            let tex_transform_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTexTransform\0".as_ptr() as *const GLchar); ck();
            let tex_translation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTexTranslation\0".as_ptr() as *const GLchar); ck();
            let gridline_color_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uGridlineColor\0".as_ptr() as *const GLchar); ck();
            let bg_color_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uBGColor\0".as_ptr() as *const GLchar); ck();
            GridProgram {
                program,
                position_attribute,
                transform_uniform,
                translation_uniform,
                tex_transform_uniform,
                tex_translation_uniform,
                gridline_color_uniform,
                bg_color_uniform,
            }
        }
    }
}

struct CheckProgram {
    program: Program,
    position_attribute: GLint,
    transform_uniform: GLint,
    translation_uniform: GLint,
    tex_transform_uniform: GLint,
    tex_translation_uniform: GLint,
    rotation_uniform: GLint,
    color_a_uniform: GLint,
    color_b_uniform: GLint,
    viewport_origin_uniform: GLint,
    radius_uniform: GLint,
    camera_position_uniform: GLint,
    light_position_uniform: GLint,
    sphere_position_uniform: GLint,
}

impl CheckProgram {
    fn new(gl_texture_target: GLenum) -> CheckProgram {
        let vertex_shader = Shader::new("quad", ShaderKind::Vertex, gl_texture_target);
        let fragment_shader = Shader::new("check", ShaderKind::Fragment, gl_texture_target);
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object,
                                      b"aPosition\0".as_ptr() as *const GLchar); ck();
            let transform_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTransform\0".as_ptr() as *const GLchar); ck();
            let translation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTranslation\0".as_ptr() as *const GLchar); ck();
            let tex_transform_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTexTransform\0".as_ptr() as *const GLchar); ck();
            let tex_translation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uTexTranslation\0".as_ptr() as *const GLchar); ck();
            let rotation_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uRotation\0".as_ptr() as *const GLchar); ck();
            let color_a_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uColorA\0".as_ptr() as *const GLchar); ck();
            let color_b_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uColorB\0".as_ptr() as *const GLchar); ck();
            let viewport_origin_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uViewportOrigin\0".as_ptr() as *const GLchar); ck();
            let radius_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uRadius\0".as_ptr() as *const GLchar); ck();
            let camera_position_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uCameraPosition\0".as_ptr() as *const GLchar); ck();
            let light_position_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uLightPosition\0".as_ptr() as *const GLchar); ck();
            let sphere_position_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uSpherePosition\0".as_ptr() as *const GLchar); ck();
            CheckProgram {
                program,
                position_attribute,
                transform_uniform,
                translation_uniform,
                tex_transform_uniform,
                tex_translation_uniform,
                rotation_uniform,
                color_a_uniform,
                color_b_uniform,
                viewport_origin_uniform,
                radius_uniform,
                light_position_uniform,
                camera_position_uniform,
                sphere_position_uniform,
            }
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
