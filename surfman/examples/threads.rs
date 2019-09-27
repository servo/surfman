// surfman/examples/threads.rs
//
// This example demonstrates how to create a multithreaded OpenGL application using `surfman`.

use crate::common::{Buffer, Program, Shader, ShaderKind, ck};

use euclid::default::Size2D;
use gl::types::{GLchar, GLenum, GLint, GLuint, GLvoid};
use sdl2::event::Event;
use sdl2::hint;
use sdl2::keyboard::Keycode;
use sdl2::video::{GLProfile, SwapInterval};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use surfman::{Adapter, ContextDescriptor, Device, GLApi, Surface};

mod common;

const WINDOW_WIDTH:  i32 = 640;
const WINDOW_HEIGHT: i32 = 480;

const SUBSCREEN_WIDTH:  i32 = 256;
const SUBSCREEN_HEIGHT: i32 = 256;

const CHECK_SIZE: f32 = 16.0;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 0, 1, 1, 1 ];

static BLIT_TRANSLATION: [f32; 2] = [0.0, 0.0];
static BLIT_TRANSFORM: [f32; 4] = [
    SUBSCREEN_WIDTH as f32 / WINDOW_WIDTH as f32, 0.0,
    0.0, SUBSCREEN_HEIGHT as f32 / WINDOW_HEIGHT as f32,
];

static CHECK_TRANSFORM: [f32; 4] = [
    SUBSCREEN_WIDTH as f32 / CHECK_SIZE as f32, 0.0,
    0.0, SUBSCREEN_HEIGHT as f32 / CHECK_SIZE as f32,
];

static IDENTITY_TRANSFORM: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
static ZERO_TRANSLATION:   [f32; 2] = [0.0, 0.0];

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
    let vertex_array = BlitVertexArray::new(device.surface_gl_texture_target());

    // Fetch our initial surface.
    let mut surface = main_from_worker_receiver.recv().unwrap();
    let mut texture = device.create_surface_texture(&mut context, surface).unwrap();

    // Enter main render loop.
    let mut animation = Animation::new(0.75, 0.003);
    loop {
        // Send back our old surface, and fetch a new one.
        surface = device.destroy_surface_texture(&mut context, texture).unwrap();
        main_to_worker_sender.send(surface).unwrap();
        surface = main_from_worker_receiver.recv().unwrap();
        texture = device.create_surface_texture(&mut context, surface).unwrap();

        unsafe {
            let value = animation.tick();
            gl::ClearColor(value, 0.0, 0.0, 1.0); ck();
            gl::Clear(gl::COLOR_BUFFER_BIT); ck();

            gl::BindVertexArray(vertex_array.object); ck();
            gl::UseProgram(vertex_array.blit_program.program.object); ck();
            gl::UniformMatrix2fv(vertex_array.blit_program.transform_uniform,
                                 1,
                                 gl::FALSE,
                                 BLIT_TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.blit_program.translation_uniform,
                           1,
                           BLIT_TRANSLATION.as_ptr());
            gl::UniformMatrix2fv(vertex_array.blit_program.tex_transform_uniform,
                                 1,
                                 gl::FALSE,
                                 IDENTITY_TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.blit_program.tex_translation_uniform,
                           1,
                           ZERO_TRANSLATION.as_ptr());
            gl::ActiveTexture(gl::TEXTURE0); ck();
            gl::BindTexture(device.surface_gl_texture_target(), texture.gl_texture()); ck();
            gl::Uniform1i(vertex_array.blit_program.source_uniform, 0); ck();
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4); ck();
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
                 worker_to_main_sender: Sender<Surface>,
                 worker_from_main_receiver: Receiver<Surface>) {
    // Open the device, create a context, and make it current.
    let size = Size2D::new(SUBSCREEN_WIDTH, SUBSCREEN_HEIGHT);
    let mut device = Device::new(&adapter).unwrap();
    let mut context = device.create_context(&context_descriptor, &size).unwrap();
    device.make_context_current(&context).unwrap();

    // Set up GL objects and state.
    let vertex_array = CheckVertexArray::new(device.surface_gl_texture_target());

    // Send an initial surface back to the main thread.
    let surface = device.create_surface(&context, &size).unwrap();
    worker_to_main_sender.send(surface).unwrap();

    let mut offset = 0.0;
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
                                 IDENTITY_TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.check_program.translation_uniform,
                           1,
                           ZERO_TRANSLATION.as_ptr());
            gl::UniformMatrix2fv(vertex_array.check_program.tex_transform_uniform,
                                 1,
                                 gl::FALSE,
                                 CHECK_TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.check_program.tex_translation_uniform,
                           1,
                           [offset, 0.0].as_ptr());
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4); ck();
        }

        let new_surface = worker_from_main_receiver.recv().unwrap();
        let old_surface = device.replace_context_surface(&mut context, new_surface).unwrap();
        worker_to_main_sender.send(old_surface).unwrap();

        offset += 1.0 / CHECK_SIZE;
    }
}

struct Animation {
    value: f32,
    delta: f32,
}

impl Animation {
    fn new(value: f32, delta: f32) -> Animation {
        Animation { value, delta }
    }

    fn tick(&mut self) -> f32 {
        let old_value = self.value;
        self.value += self.delta;
        if self.value > 1.0 && self.delta > 0.0 {
            self.value = 1.0;
            self.delta = -self.delta;
        } else if self.value < 0.0 && self.delta < 0.0 {
            self.value = 0.0;
            self.delta = -self.delta;
        }
        old_value
    }
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

struct CheckProgram {
    program: Program,
    position_attribute: GLint,
    transform_uniform: GLint,
    translation_uniform: GLint,
    tex_transform_uniform: GLint,
    tex_translation_uniform: GLint,
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
            CheckProgram {
                program,
                position_attribute,
                transform_uniform,
                translation_uniform,
                tex_transform_uniform,
                tex_translation_uniform,
            }
        }
    }
}
