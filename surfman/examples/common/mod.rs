// examples/common/mod.rs
//
// OpenGL convenience wrappers used in the examples.

use gl;
use gl::types::{GLchar, GLenum, GLint, GLuint};
use std::fs::File;
use std::io::Read;
use std::os::raw::c_void;
use std::ptr;
use surfman::GLApi;

pub struct Program {
    pub object: GLuint,
    #[allow(dead_code)]
    vertex_shader: Shader,
    #[allow(dead_code)]
    fragment_shader: Shader,
}

impl Program {
    pub fn new(vertex_shader: Shader, fragment_shader: Shader) -> Program {
        unsafe {
            let program = gl::CreateProgram();
            ck();
            gl::AttachShader(program, vertex_shader.object);
            ck();
            gl::AttachShader(program, fragment_shader.object);
            ck();
            gl::LinkProgram(program);
            ck();
            Program {
                object: program,
                vertex_shader,
                fragment_shader,
            }
        }
    }
}

pub struct Shader {
    object: GLuint,
}

impl Shader {
    pub fn new(
        name: &str,
        kind: ShaderKind,
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> Shader {
        let mut source = vec![];
        match gl_api {
            GLApi::GL => source.extend_from_slice(b"#version 330\n"),
            GLApi::GLES => source.extend_from_slice(b"#version 300 es\n"),
        }
        match gl_texture_target {
            gl::TEXTURE_2D => {}
            gl::TEXTURE_RECTANGLE => source.extend_from_slice(b"#define SAMPLER_RECT\n"),
            _ => {}
        }
        resource_loader.slurp(&mut source, &format!("{}.{}.glsl", name, kind.extension()));

        unsafe {
            let shader = gl::CreateShader(kind.to_gl());
            ck();
            gl::ShaderSource(
                shader,
                1,
                &(source.as_ptr() as *const GLchar),
                &(source.len() as GLint),
            );
            ck();
            gl::CompileShader(shader);
            ck();

            let mut compile_status = 0;
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut compile_status);
            ck();
            if compile_status != gl::TRUE as GLint {
                let mut info_log_length = 0;
                gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut info_log_length);
                let mut info_log = vec![0; info_log_length as usize + 1];
                gl::GetShaderInfoLog(
                    shader,
                    info_log_length,
                    ptr::null_mut(),
                    info_log.as_mut_ptr() as *mut _,
                );
                eprintln!(
                    "Failed to compile shader:\n{}",
                    String::from_utf8_lossy(&info_log)
                );
                panic!("Shader compilation failed!");
            }
            debug_assert_eq!(compile_status, gl::TRUE as GLint);

            Shader { object: shader }
        }
    }
}

pub struct Buffer {
    pub object: GLuint,
}

impl Buffer {
    pub fn from_data(data: &[u8]) -> Buffer {
        unsafe {
            let mut buffer = 0;
            gl::GenBuffers(1, &mut buffer);
            ck();
            gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
            ck();
            gl::BufferData(
                gl::ARRAY_BUFFER,
                data.len() as isize,
                data.as_ptr() as *const c_void,
                gl::STATIC_DRAW,
            );
            ck();
            Buffer { object: buffer }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ShaderKind {
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

pub trait ResourceLoader {
    fn slurp(&self, dest: &mut Vec<u8>, filename: &str);
}

#[allow(dead_code)]
pub struct FilesystemResourceLoader;

impl ResourceLoader for FilesystemResourceLoader {
    fn slurp(&self, dest: &mut Vec<u8>, filename: &str) {
        let path = format!("resources/examples/{}", filename);
        File::open(&path)
            .expect("Failed to open file!")
            .read_to_end(dest)
            .unwrap();
    }
}

pub fn ck() {
    unsafe {
        debug_assert_eq!(gl::GetError(), gl::NO_ERROR);
    }
}
