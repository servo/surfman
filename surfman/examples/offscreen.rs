// surfman/surfman/examples/offscreen.rs
//
//! This example demonstrates how to create an off-screen context and render into it using
//! `surfman` alone, without any other windowing libraries.

use crate::common::{ck, Buffer, FilesystemResourceLoader, Program, Shader, ShaderKind};

use clap::{App, Arg};
use euclid::default::Size2D;
use gl;
use gl::types::{GLchar, GLenum, GLint, GLuint, GLvoid};
use png::{BitDepth, ColorType, Encoder};
use std::fs::File;
use std::mem;
use std::path::Path;
use std::slice;
use surfman::{Connection, ContextAttributeFlags, ContextAttributes, GLApi, GLVersion};
use surfman::{SurfaceAccess, SurfaceType};

mod common;

const FRAMEBUFFER_WIDTH: i32 = 640;
const FRAMEBUFFER_HEIGHT: i32 = 480;

#[derive(Clone, Copy)]
#[repr(C)]
struct Vertex {
    x: f32,
    y: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

static TRI_VERTICES: [Vertex; 3] = [
    Vertex {
        x: 0.0,
        y: 0.5,
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    },
    Vertex {
        x: 0.5,
        y: -0.5,
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    },
    Vertex {
        x: -0.5,
        y: -0.5,
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    },
];

static APP_NAME: &'static str = "surfman offscreen example";

fn main() {
    let matches = App::new(APP_NAME)
        .arg(
            Arg::with_name("hardware")
                .short("H")
                .long("hardware")
                .help("Use hardware rendering"),
        )
        .arg(
            Arg::with_name("software")
                .short("s")
                .long("software")
                .conflicts_with("hardware")
                .help("Use software rendering"),
        )
        .arg(
            Arg::with_name("OUTPUT")
                .required(true)
                .index(1)
                .help("Output PNG file"),
        )
        .get_matches();

    let connection = Connection::new().unwrap();

    let adapter = if matches.is_present("software") {
        connection.create_software_adapter().unwrap()
    } else if matches.is_present("hardware") {
        connection.create_hardware_adapter().unwrap()
    } else {
        connection.create_adapter().unwrap()
    };

    let output_path = Path::new(matches.value_of("OUTPUT").unwrap()).to_owned();
    let output_file = File::create(output_path).unwrap();

    let mut device = connection.create_device(&adapter).unwrap();

    let context_attributes = ContextAttributes {
        version: GLVersion::new(3, 3),
        flags: ContextAttributeFlags::empty(),
    };
    let context_descriptor = device
        .create_context_descriptor(&context_attributes)
        .unwrap();
    let mut context = device.create_context(&context_descriptor, None).unwrap();
    let surface = device
        .create_surface(
            &context,
            SurfaceAccess::GPUOnly,
            SurfaceType::Generic {
                size: Size2D::new(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT),
            },
        )
        .unwrap();
    device
        .bind_surface_to_context(&mut context, surface)
        .unwrap();

    device.make_context_current(&context).unwrap();
    gl::load_with(|symbol_name| device.get_proc_address(&context, symbol_name));

    let mut pixels: Vec<u8> = vec![0; FRAMEBUFFER_WIDTH as usize * FRAMEBUFFER_HEIGHT as usize * 4];
    let tri_vertex_array = TriVertexArray::new(device.gl_api(), device.surface_gl_texture_target());

    unsafe {
        let surface_info = device.context_surface_info(&context).unwrap().unwrap();
        gl::BindFramebuffer(gl::FRAMEBUFFER, surface_info.framebuffer_object);
        gl::Viewport(0, 0, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT);
        ck();
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        ck();
        gl::Clear(gl::COLOR_BUFFER_BIT);
        ck();
        gl::BindVertexArray(tri_vertex_array.object);
        ck();
        gl::UseProgram(tri_vertex_array.tri_program.program.object);
        ck();
        gl::DrawArrays(gl::TRIANGLES, 0, 3);
        ck();
        gl::Flush();
        ck();

        gl::ReadPixels(
            0,
            0,
            FRAMEBUFFER_WIDTH,
            FRAMEBUFFER_HEIGHT,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            pixels.as_mut_ptr() as *mut GLvoid,
        );
        ck();
    }

    device.destroy_context(&mut context).unwrap();

    let mut encoder = Encoder::new(
        output_file,
        FRAMEBUFFER_WIDTH as u32,
        FRAMEBUFFER_HEIGHT as u32,
    );
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut image_writer = encoder.write_header().unwrap();
    image_writer.write_image_data(&pixels).unwrap();
}

struct TriVertexArray {
    object: GLuint,
    tri_program: TriProgram,
    #[allow(dead_code)]
    vertex_buffer: Buffer,
}

impl TriVertexArray {
    fn new(gl_api: GLApi, gl_texture_target: GLenum) -> TriVertexArray {
        let tri_program = TriProgram::new(gl_api, gl_texture_target);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array);
            ck();
            gl::BindVertexArray(vertex_array);
            ck();

            let vertex_buffer = Buffer::from_data(slice::from_raw_parts(
                TRI_VERTICES.as_ptr() as *const u8,
                mem::size_of::<Vertex>() * 3,
            ));

            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.object);
            ck();
            gl::VertexAttribPointer(
                tri_program.position_attribute as GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                12,
                0 as *const GLvoid,
            );
            ck();
            gl::VertexAttribPointer(
                tri_program.color_attribute as GLuint,
                4,
                gl::UNSIGNED_BYTE,
                gl::TRUE,
                12,
                8 as *const GLvoid,
            );
            ck();
            gl::EnableVertexAttribArray(tri_program.position_attribute as GLuint);
            ck();
            gl::EnableVertexAttribArray(tri_program.color_attribute as GLuint);
            ck();

            TriVertexArray {
                object: vertex_array,
                tri_program,
                vertex_buffer,
            }
        }
    }
}

struct TriProgram {
    program: Program,
    position_attribute: GLint,
    color_attribute: GLint,
}

impl TriProgram {
    fn new(gl_api: GLApi, gl_texture_target: GLenum) -> TriProgram {
        let vertex_shader = Shader::new(
            "tri",
            ShaderKind::Vertex,
            gl_api,
            gl_texture_target,
            &FilesystemResourceLoader,
        );
        let fragment_shader = Shader::new(
            "tri",
            ShaderKind::Fragment,
            gl_api,
            gl_texture_target,
            &FilesystemResourceLoader,
        );
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object, b"aPosition\0".as_ptr() as *const GLchar);
            ck();
            let color_attribute =
                gl::GetAttribLocation(program.object, "aColor\0".as_ptr() as *const GLchar);
            ck();
            TriProgram {
                program,
                position_attribute,
                color_attribute,
            }
        }
    }
}
