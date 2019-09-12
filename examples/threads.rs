// examples/threads.rs
//
// This example demonstrates how to create a multithreaded OpenGL
// application using `surfman`.

use euclid::default::Size2D;
use gl::types::{GLchar, GLenum, GLint, GLuint, GLvoid};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::fs::File;
use std::io::Read;
use std::os::raw::c_void;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use surfman::{Adapter, ContextDescriptor, Device, Surface, SurfaceTexture};

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 0, 1, 1, 1];

static TRANSFORM: [f32; 4] = [0.5, 0.0, 0.0, 0.5];
static TRANSLATION: [f32; 2] = [0.0, 0.0];

fn main() {
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context.
    let gl_attributes = video.gl_attr();
    gl_attributes.set_context_profile(GLProfile::Core);
    gl_attributes.set_context_version(3, 3);

    // Open a window.
    let window = video.window("Multithreaded example", 1067, 800).opengl().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    // Create the GL context in SDL, and make it current.
    let gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).unwrap();

    // Create surfman objects corresponding to that SDL context.
    let (device, mut context) = unsafe {
        Device::from_current_context().unwrap()
    };
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
    let vertex_array = BlitVertexArray::new();

    // Fetch our initial surface.
    let mut surface = main_from_worker_receiver.recv().unwrap();
    let mut texture = device.create_surface_texture(&mut context, surface).unwrap();

    // Enter main render loop.
    loop {
        // Send back our old surface, and fetch a new one.
        surface = device.destroy_surface_texture(&mut context, texture).unwrap();
        main_to_worker_sender.send(surface).unwrap();
        surface = main_from_worker_receiver.recv().unwrap();
        texture = device.create_surface_texture(&mut context, surface).unwrap();

        unsafe {
            gl::ClearColor(0.0, 0.0, 0.5, 1.0); ck();
            gl::Clear(gl::COLOR_BUFFER_BIT); ck();

            gl::BindVertexArray(vertex_array.object); ck();
            gl::UseProgram(vertex_array.blit_program.program.object); ck();
            gl::UniformMatrix2fv(vertex_array.blit_program.transform_uniform,
                                 1,
                                 gl::FALSE,
                                 TRANSFORM.as_ptr());
            gl::Uniform2fv(vertex_array.blit_program.translation_uniform, 1, TRANSLATION.as_ptr());
            gl::ActiveTexture(gl::TEXTURE0); ck();
            gl::BindTexture(SurfaceTexture::gl_texture_target(), texture.gl_texture()); ck();
            gl::Uniform1i(vertex_array.blit_program.source_uniform, 0); ck();
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4); ck();
        }

        window.gl_swap_window();

        match event_pump.poll_event() {
            Some(Event::Quit {..}) |
            Some(Event::KeyDown { keycode: Some(Keycode::Escape), .. }) => return,
            _ => {}
        }
    }
}

fn worker_thread(adapter: Adapter,
                 context_descriptor: ContextDescriptor,
                 worker_to_main_sender: Sender<Surface>,
                 worker_from_main_receiver: Receiver<Surface>) {
    // Open the device, create a context, and make it current.
    let mut device = Device::new(&adapter).unwrap();
    let mut context = device.create_context(&context_descriptor, &Size2D::new(256, 256)).unwrap();
    device.make_context_current(&context).unwrap();

    // Send an initial surface back to the main thread.
    let surface = device.create_surface(&context, &Size2D::new(256, 256)).unwrap();
    worker_to_main_sender.send(surface).unwrap();

    let (mut color, mut delta) = (0.0, 0.001);
    loop {
        // Render to the surface.
        unsafe {
            let framebuffer_object = device.context_surface_framebuffer_object(&context).unwrap();
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl::Viewport(0, 0, 256, 256);

            gl::ClearColor(0.0, color, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Flush();
        }

        let new_surface = worker_from_main_receiver.recv().unwrap();
        let old_surface = device.replace_context_color_surface(&mut context, new_surface).unwrap();
        worker_to_main_sender.send(old_surface).unwrap();

        color += delta;
        if color >= 1.0 && delta > 0.0 {
            color = 1.0;
            delta = -delta;
        } else if color <= 0.0 && delta < 0.0 {
            color = 0.0;
            delta = -delta;
        }
    }
}

struct BlitVertexArray {
    object: GLuint,
    blit_program: BlitProgram,
    #[allow(dead_code)]
    vertex_buffer: Buffer,
}

impl BlitVertexArray {
    fn new() -> BlitVertexArray {
        let blit_program = BlitProgram::new();
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array); ck();
            gl::BindVertexArray(vertex_array); ck();

            let vertex_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.object); ck();
            gl::VertexAttribPointer(blit_program.position_attribute as GLuint,
                                    2,
                                    gl::UNSIGNED_BYTE,
                                    gl::FALSE,
                                    2,
                                    0 as *const GLvoid); ck();
            gl::EnableVertexAttribArray(blit_program.position_attribute as GLuint); ck();

            BlitVertexArray { object: vertex_array, blit_program, vertex_buffer }
        }
    }
}

struct BlitProgram {
    program: Program,
    position_attribute: GLint,
    transform_uniform: GLint,
    translation_uniform: GLint,
    source_uniform: GLint,
}

impl BlitProgram {
    fn new() -> BlitProgram {
        let vertex_shader = Shader::new("quad", ShaderKind::Vertex);
        let fragment_shader = Shader::new("blit", ShaderKind::Fragment);
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
            let source_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uSource\0".as_ptr() as *const GLchar); ck();
            BlitProgram {
                program,
                position_attribute,
                transform_uniform,
                translation_uniform,
                source_uniform,
            }
        }
    }
}

struct Program {
    object: GLuint,
    #[allow(dead_code)]
    vertex_shader: Shader,
    #[allow(dead_code)]
    fragment_shader: Shader,
}

impl Program {
    fn new(vertex_shader: Shader, fragment_shader: Shader) -> Program {
        unsafe {
            let program = gl::CreateProgram(); ck();
            gl::AttachShader(program, vertex_shader.object); ck();
            gl::AttachShader(program, fragment_shader.object); ck();
            gl::LinkProgram(program); ck();
            Program { object: program, vertex_shader, fragment_shader }
        }
    }
}

struct Shader {
    object: GLuint,
}

impl Shader {
    fn new(name: &str, kind: ShaderKind) -> Shader {
        let mut source = vec![];
        let path = format!("resources/examples/{}.{}.glsl", name, kind.extension());
        File::open(&path).expect("Failed to open shader source!")
                         .read_to_end(&mut source)
                         .unwrap();
        unsafe {
            let shader = gl::CreateShader(kind.to_gl()); ck();
            gl::ShaderSource(shader,
                             1,
                             &(source.as_ptr() as *const GLchar),
                             &(source.len() as GLint)); ck();
            gl::CompileShader(shader); ck();

            let mut compile_status = 0;
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut compile_status); ck();
            debug_assert_eq!(compile_status, gl::TRUE as GLint);

            Shader { object: shader }
        }
    }
}

struct Buffer {
    object: GLuint,
}

impl Buffer {
    fn from_data(data: &[u8]) -> Buffer {
        unsafe {
            let mut buffer = 0;
            gl::GenBuffers(1, &mut buffer); ck();
            gl::BindBuffer(gl::ARRAY_BUFFER, buffer); ck();
            gl::BufferData(gl::ARRAY_BUFFER,
                           data.len() as isize,
                           data.as_ptr() as *const c_void,
                           gl::STATIC_DRAW); ck();
            Buffer { object: buffer }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ShaderKind {
    Vertex,
    Fragment,
}

impl ShaderKind {
    fn extension(self) -> &'static str {
        match self {
            ShaderKind::Vertex => "vs",
            ShaderKind::Fragment => "fs",
        }
    }

    fn to_gl(self) -> GLenum {
        match self {
            ShaderKind::Vertex => gl::VERTEX_SHADER,
            ShaderKind::Fragment => gl::FRAGMENT_SHADER,
        }
    }
}

fn ck() {
    unsafe {
        debug_assert_eq!(gl::GetError(), gl::NO_ERROR);
    }
}
