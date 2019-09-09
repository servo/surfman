// examples/boing.rs
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
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;
use surfman::{Adapter, ContextAttributeFlags, ContextAttributes, Device, GLApi, GLFlavor};
use surfman::{GLVersion, Surface, SurfaceDescriptor, SurfaceTexture};

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 0, 1, 1, 1];

fn main() {
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context.
    let gl_attributes = video.gl_attr();
    gl_attributes.set_context_profile(GLProfile::Core);
    gl_attributes.set_context_version(3, 3);

    // Open a window.
    let window = video.window("Boing ball example", 1067, 800).opengl().build().unwrap();
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

    // Set up communication channels, and spawn our worker thread.
    let (worker_to_main_sender, main_from_worker_receiver) = mpsc::channel();
    thread::spawn(move || worker_thread(adapter, worker_to_main_sender));

    // Set up GL objects and state.
    let vertex_array = BlitVertexArray::new();

    /*
    let mut ball_texture = 0;
    unsafe {
        gl::GenTextures(1, &mut ball_texture); ck();
        gl::ActiveTexture(gl::TEXTURE0); ck();
        gl::BindTexture(gl::TEXTURE_2D, ball_texture); ck();
        let mut pixels: Vec<u8> = vec![0; 256 * 256 * 4];
        for i in 0..(256 * 256) {
            pixels[i * 4 + 0] = 128;
            pixels[i * 4 + 3] = 255;
        }
        gl::TexImage2D(gl::TEXTURE_2D,
                       0,
                       gl::RGBA8 as GLint,
                       256,
                       256,
                       0,
                       gl::RGBA,
                       gl::UNSIGNED_BYTE,
                       pixels.as_ptr() as *const c_void); ck();
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
    }
    */

    // Fetch a surface.
    let ball_surface = main_from_worker_receiver.recv().unwrap();
    let ball_texture = device.create_surface_texture(&mut context, ball_surface).unwrap();

    // Enter main render loop.
    loop {
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0); ck();
            gl::Clear(gl::COLOR_BUFFER_BIT); ck();

            gl::BindVertexArray(vertex_array.object); ck();
            gl::UseProgram(vertex_array.blit_program.program.object); ck();
            gl::ActiveTexture(gl::TEXTURE0); ck();
            gl::BindTexture(SurfaceTexture::gl_texture_target(), ball_texture.gl_texture()); ck();
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

fn worker_thread(adapter: Adapter, worker_to_main_sender: Sender<Surface>) {
    // Open the device, and create a context.
    let flavor = GLFlavor { api: GLApi::GL, version: GLVersion::new(3, 3) };
    let context_attributes = ContextAttributes { flags: ContextAttributeFlags::empty(), flavor };
    let mut device = Device::new(&adapter).unwrap();
    let mut context = device.create_context(&context_attributes).unwrap();

    // Create a surface, and attach it to the context.
    let surface_size = Size2D::new(256, 256);
    let surface_descriptor =
        SurfaceDescriptor::from_context_attributes_and_size(&context_attributes, &surface_size);
    let surface = device.create_surface_from_descriptor(&mut context,
                                                        &surface_descriptor).unwrap();
    device.replace_context_color_surface(&mut context, surface).unwrap();

    // Make the context current.
    device.make_context_current(&context).unwrap();

    // Render to the surface.
    unsafe {
        let framebuffer_object = device.context_surface_framebuffer_object(&context).unwrap();
        gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
        gl::Viewport(0, 0, 256, 256);

        gl::ClearColor(0.0, 0.5, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::Flush();
    }

    // Make a dummy service.
    // FIXME(pcwalton): Bad! Change the API!
    let surface = device.create_surface_from_descriptor(&mut context,
                                                        &surface_descriptor).unwrap();
    let surface = device.replace_context_color_surface(&mut context, surface).unwrap().unwrap();
    worker_to_main_sender.send(surface).unwrap();

    // FIXME(pcwalton)
    loop {
        thread::sleep(Duration::from_secs(1));
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
            let source_uniform =
                gl::GetUniformLocation(program.object,
                                       b"uSource\0".as_ptr() as *const GLchar); ck();
            BlitProgram { program, position_attribute, source_uniform }
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
