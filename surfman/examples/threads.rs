// surfman/examples/threads.rs
//
// This example demonstrates how to create a multithreaded OpenGL application using `surfman`.

use self::common::{ck, Buffer, Program, ResourceLoader, Shader, ShaderKind};

use euclid::default::{Point2D, Rect, Size2D, Vector2D};
use gl::types::{GLchar, GLenum, GLint, GLuint, GLvoid};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use surfman::{declare_surfman, SurfaceAccess, SurfaceTexture, SurfaceType};
use surfman::{Adapter, Connection, Context, ContextDescriptor, Device, GLApi, Surface};

#[cfg(not(target_os = "android"))]
use self::common::FilesystemResourceLoader;

#[cfg(not(target_os = "android"))]
use surfman::{ContextAttributeFlags, ContextAttributes, GLVersion};
#[cfg(not(target_os = "android"))]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{DeviceEvent, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};

pub mod common;

declare_surfman!();

const WINDOW_WIDTH: i32 = 800;
const WINDOW_HEIGHT: i32 = 600;

const SUBSCREEN_WIDTH: i32 = 256;
const SUBSCREEN_HEIGHT: i32 = 256;

const BALL_WIDTH: i32 = 192;
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

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 0, 1, 1, 1];

static CHECK_TRANSFORM: [f32; 4] = [
    SUBSCREEN_WIDTH as f32 / CHECK_SIZE as f32,
    0.0,
    0.0,
    SUBSCREEN_HEIGHT as f32 / CHECK_SIZE as f32,
];

static IDENTITY_TRANSFORM: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
static ZERO_TRANSLATION: [f32; 2] = [0.0, 0.0];

static NDC_TRANSFORM: [f32; 4] = [2.0, 0.0, 0.0, 2.0];
static NDC_TRANSLATION: [f32; 2] = [-1.0, -1.0];

static CHECK_COLOR_A: [f32; 4] = [0.8, 0.0, 0.0, 1.0];
static CHECK_COLOR_B: [f32; 4] = [0.9, 0.9, 0.9, 1.0];

static CAMERA_POSITION: [f32; 3] = [400.0, 300.0, -1000.0];
static LIGHT_POSITION: [f32; 3] = [600.0, 450.0, -500.0];

static GRIDLINE_COLOR: [f32; 4] = [
    (0x9e as f32) / 255.0,
    (0x2b as f32) / 255.0,
    (0x86 as f32) / 255.0,
    1.0,
];
static BACKGROUND_COLOR: [f32; 4] = [
    (0xaa as f32) / 255.0,
    (0xaa as f32) / 255.0,
    (0xaa as f32) / 255.0,
    1.0,
];

#[cfg(not(target_os = "android"))]
fn main() {
    let event_loop = EventLoop::new();
    let dpi = event_loop.primary_monitor().unwrap().scale_factor();
    let window_size = Size2D::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let logical_size =
        PhysicalSize::new(window_size.width, window_size.height)
        .to_logical::<f64>(dpi);

    let window = WindowBuilder::new()
        .with_title("Multithreaded example")
        .with_inner_size(logical_size)
        .build(&event_loop)
        .unwrap();

    window.set_visible(true);

    let connection = Connection::from_winit_window(&window).unwrap();
    let native_widget = connection
        .create_native_widget_from_winit_window(&window)
        .unwrap();
    let adapter = connection.create_low_power_adapter().unwrap();
    let mut device = connection.create_device(&adapter).unwrap();

    let context_attributes = ContextAttributes {
        version: GLVersion::new(3, 0),
        flags: ContextAttributeFlags::ALPHA,
    };
    let context_descriptor = device
        .create_context_descriptor(&context_attributes)
        .unwrap();

    let surface_type = SurfaceType::Widget { native_widget };
    let mut context = device.create_context(&context_descriptor, None).unwrap();
    let surface = device
        .create_surface(&context, SurfaceAccess::GPUOnly, surface_type)
        .unwrap();
    device
        .bind_surface_to_context(&mut context, surface)
        .unwrap();
    device.make_context_current(&context).unwrap();

    let mut app = App::new(
        connection,
        adapter,
        device,
        context,
        Box::new(FilesystemResourceLoader),
        window_size,
    );

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        }
        | Event::DeviceEvent {
            event:
                DeviceEvent::Key(KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    ..
                }),
            ..
        } => *control_flow = ControlFlow::Exit,
        _ => { app.tick(true); *control_flow = ControlFlow::Poll; }
    });

}
pub struct App {
    main_from_worker_receiver: Receiver<Frame>,
    main_to_worker_sender: Sender<Surface>,
    grid_vertex_array: GridVertexArray,
    blit_vertex_array: BlitVertexArray,
    device: Device,
    context: Context,
    texture: Option<SurfaceTexture>,
    frame: Frame,
    window_size: Size2D<i32>,
}

impl Drop for App {
    fn drop(&mut self) {
        self.device.destroy_context(&mut self.context).unwrap();
    }
}

impl App {

    pub fn new(
        connection: Connection,
        adapter: Adapter,
        device: Device,
        mut context: Context,
        resource_loader: Box<dyn ResourceLoader + Send>,
        window_size: Size2D<i32>,
    ) -> App {
        let context_descriptor = device.context_descriptor(&context);

        gl::load_with(|symbol_name| device.get_proc_address(&context, symbol_name));

        // Set up GL objects and state.
        let gl_api = device.gl_api();
        let surface_gl_texture_target = device.surface_gl_texture_target();
        let grid_vertex_array =
            GridVertexArray::new(gl_api, surface_gl_texture_target, &*resource_loader);
        let blit_vertex_array =
            BlitVertexArray::new(gl_api, surface_gl_texture_target, &*resource_loader);

        // Set up communication channels, and spawn our worker thread.
        let (worker_to_main_sender, main_from_worker_receiver) = mpsc::channel();
        let (main_to_worker_sender, worker_from_main_receiver) = mpsc::channel();
        thread::spawn(move || {
            worker_thread(
                connection,
                adapter,
                context_descriptor,
                window_size,
                resource_loader,
                worker_to_main_sender,
                worker_from_main_receiver,
            )
        });

        // Fetch our initial surface.
        let mut frame = match main_from_worker_receiver.recv() {
            Err(_) => panic!(),
            Ok(frame) => frame,
        };
        let texture = Some(
            device
                .create_surface_texture(&mut context, frame.surface.take().unwrap())
                .unwrap(),
        );

        App {
            main_from_worker_receiver,
            main_to_worker_sender,
            grid_vertex_array,
            blit_vertex_array,
            device,
            texture,
            frame,
            context,
            window_size,
        }
    }

    pub fn tick(&mut self, present: bool) {
        // Send back our old surface.
        let surface = self
            .device
            .destroy_surface_texture(&mut self.context, self.texture.take().unwrap())
            .unwrap();
        self.main_to_worker_sender.send(surface).unwrap();

        // Fetch a new frame.
        self.frame = self.main_from_worker_receiver.recv().unwrap();

        // Wrap it in a texture.
        self.texture = Some(
            self.device
                .create_surface_texture(&mut self.context, self.frame.surface.take().unwrap())
                .unwrap(),
        );

        unsafe {
            self.device.make_context_current(&self.context).unwrap();

            let framebuffer_object = match self.device.context_surface_info(&self.context) {
                Ok(Some(surface_info)) => surface_info.framebuffer_object,
                _ => 0,
            };

            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl::Viewport(0, 0, self.window_size.width, self.window_size.height);

            gl::ClearColor(0.0, 0.0, 1.0, 1.0);
            ck();
            gl::Clear(gl::COLOR_BUFFER_BIT);
            ck();

            // Draw gridlines.
            gl::BindVertexArray(self.grid_vertex_array.object);
            ck();
            gl::UseProgram(self.grid_vertex_array.grid_program.program.object);
            ck();
            gl::UniformMatrix2fv(
                self.grid_vertex_array.grid_program.transform_uniform,
                1,
                gl::FALSE,
                NDC_TRANSFORM.as_ptr(),
            );
            gl::Uniform2fv(
                self.grid_vertex_array.grid_program.translation_uniform,
                1,
                NDC_TRANSLATION.as_ptr(),
            );
            gl::UniformMatrix2fv(
                self.grid_vertex_array.grid_program.tex_transform_uniform,
                1,
                gl::FALSE,
                CHECK_TRANSFORM.as_ptr(),
            );
            gl::Uniform2fv(
                self.grid_vertex_array.grid_program.tex_translation_uniform,
                1,
                ZERO_TRANSLATION.as_ptr(),
            );
            gl::Uniform4fv(
                self.grid_vertex_array.grid_program.gridline_color_uniform,
                1,
                GRIDLINE_COLOR.as_ptr(),
            );
            gl::Uniform4fv(
                self.grid_vertex_array.grid_program.bg_color_uniform,
                1,
                BACKGROUND_COLOR.as_ptr(),
            );
            gl::Uniform2f(
                self.grid_vertex_array.grid_program.sphere_position_uniform,
                self.frame.sphere_position.x,
                self.frame.sphere_position.y,
            );
            gl::Uniform1f(
                self.grid_vertex_array.grid_program.radius_uniform,
                SPHERE_RADIUS,
            );
            gl::Uniform3fv(
                self.grid_vertex_array.grid_program.camera_position_uniform,
                1,
                CAMERA_POSITION.as_ptr(),
            );
            gl::Uniform3fv(
                self.grid_vertex_array.grid_program.light_position_uniform,
                1,
                LIGHT_POSITION.as_ptr(),
            );
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            ck();

            // Draw subscreen.
            let blit_transform: [f32; 4] = [
                SUBSCREEN_WIDTH as f32 / self.window_size.width as f32 * 2.0,
                0.0,
                0.0,
                SUBSCREEN_HEIGHT as f32 / self.window_size.height as f32 * 2.0,
            ];

            let subscreen_translation = Point2D::new(
                self.frame.viewport_origin.x / self.window_size.width as f32 * 2.0 - 1.0,
                self.frame.viewport_origin.y / self.window_size.height as f32 * 2.0 - 1.0,
            );
            gl::BindVertexArray(self.blit_vertex_array.object);
            ck();
            gl::UseProgram(self.blit_vertex_array.blit_program.program.object);
            ck();
            gl::UniformMatrix2fv(
                self.blit_vertex_array.blit_program.transform_uniform,
                1,
                gl::FALSE,
                blit_transform.as_ptr(),
            );
            gl::Uniform2fv(
                self.blit_vertex_array.blit_program.translation_uniform,
                1,
                [subscreen_translation.x, subscreen_translation.y].as_ptr(),
            );
            gl::UniformMatrix2fv(
                self.blit_vertex_array.blit_program.tex_transform_uniform,
                1,
                gl::FALSE,
                IDENTITY_TRANSFORM.as_ptr(),
            );
            gl::Uniform2fv(
                self.blit_vertex_array.blit_program.tex_translation_uniform,
                1,
                ZERO_TRANSLATION.as_ptr(),
            );
            gl::ActiveTexture(gl::TEXTURE0);
            ck();
            gl::BindTexture(
                self.device.surface_gl_texture_target(),
                self.device
                    .surface_texture_object(self.texture.as_ref().unwrap()),
            );
            gl::Uniform1i(self.blit_vertex_array.blit_program.source_uniform, 0);
            ck();
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            ck();
            gl::Disable(gl::BLEND);
        }

        if present {
            let mut surface = self
                .device
                .unbind_surface_from_context(&mut self.context)
                .unwrap()
                .unwrap();
            self.device
                .present_surface(&mut self.context, &mut surface)
                .unwrap();
            self.device
                .bind_surface_to_context(&mut self.context, surface)
                .unwrap();
        }
    }
}

fn worker_thread(
    connection: Connection,
    adapter: Adapter,
    context_descriptor: ContextDescriptor,
    window_size: Size2D<i32>,
    resource_loader: Box<dyn ResourceLoader>,
    worker_to_main_sender: Sender<Frame>,
    worker_from_main_receiver: Receiver<Surface>,
) {
    // Open the device, create a context, and make it current.
    let size = Size2D::new(SUBSCREEN_WIDTH, SUBSCREEN_HEIGHT);
    let surface_type = SurfaceType::Generic { size };
    let mut device = connection.create_device(&adapter).unwrap();
    let mut context = device.create_context(&context_descriptor, None).unwrap();
    let surface = device
        .create_surface(&context, SurfaceAccess::GPUOnly, surface_type)
        .unwrap();
    device
        .bind_surface_to_context(&mut context, surface)
        .unwrap();
    device.make_context_current(&context).unwrap();

    // Set up GL objects and state.
    let vertex_array = CheckVertexArray::new(
        device.gl_api(),
        device.surface_gl_texture_target(),
        &*resource_loader,
    );

    // Initialize our origin and size.
    let ball_origin = Point2D::new(
        window_size.width as f32 * 0.5 - BALL_WIDTH as f32 * 0.5,
        window_size.height as f32 * 0.65 - BALL_HEIGHT as f32 * 0.5,
    );
    let ball_size = Size2D::new(BALL_WIDTH as f32, BALL_HEIGHT as f32);
    let mut ball_rect = Rect::new(ball_origin, ball_size);
    let mut ball_velocity = Vector2D::new(INITIAL_VELOCITY_X, INITIAL_VELOCITY_Y);
    let subscreen_offset = (Point2D::new(SUBSCREEN_WIDTH as f32, SUBSCREEN_HEIGHT as f32)
        - Point2D::new(BALL_WIDTH as f32, BALL_HEIGHT as f32))
        * 0.5;

    // Initialize our rotation.
    let mut theta_x = INITIAL_ROTATION_X;
    let mut theta_y = INITIAL_ROTATION_Y;
    let mut theta_z = INITIAL_ROTATION_Z;

    // Send an initial surface back to the main thread.
    let surface_type = SurfaceType::Generic { size };
    let surface = Some(
        device
            .create_surface(&context, SurfaceAccess::GPUOnly, surface_type)
            .unwrap(),
    );
    worker_to_main_sender
        .send(Frame {
            surface,
            viewport_origin: ball_rect.origin - subscreen_offset,
            sphere_position: ball_rect.center(),
        })
        .unwrap();

    loop {
        // Render to the surface.
        unsafe {
            let framebuffer_object = device
                .context_surface_info(&context)
                .unwrap()
                .unwrap()
                .framebuffer_object;

            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);
            gl::Viewport(0, 0, size.width, size.height);

            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::BindVertexArray(vertex_array.object);
            ck();
            gl::UseProgram(vertex_array.check_program.program.object);
            ck();
            gl::UniformMatrix2fv(
                vertex_array.check_program.transform_uniform,
                1,
                gl::FALSE,
                NDC_TRANSFORM.as_ptr(),
            );
            gl::Uniform2fv(
                vertex_array.check_program.translation_uniform,
                1,
                NDC_TRANSLATION.as_ptr(),
            );
            gl::UniformMatrix2fv(
                vertex_array.check_program.tex_transform_uniform,
                1,
                gl::FALSE,
                NDC_TRANSFORM.as_ptr(),
            );
            gl::Uniform2fv(
                vertex_array.check_program.tex_translation_uniform,
                1,
                NDC_TRANSLATION.as_ptr(),
            );
            gl::Uniform3fv(
                vertex_array.check_program.rotation_uniform,
                1,
                [theta_x, theta_y, theta_z].as_ptr(),
            );
            gl::Uniform4fv(
                vertex_array.check_program.color_a_uniform,
                1,
                CHECK_COLOR_A.as_ptr(),
            );
            gl::Uniform4fv(
                vertex_array.check_program.color_b_uniform,
                1,
                CHECK_COLOR_B.as_ptr(),
            );
            gl::Uniform2f(
                vertex_array.check_program.viewport_origin_uniform,
                ball_rect.origin.x - subscreen_offset.x,
                ball_rect.origin.y - subscreen_offset.y,
            );
            gl::Uniform1f(vertex_array.check_program.radius_uniform, SPHERE_RADIUS);
            gl::Uniform3fv(
                vertex_array.check_program.camera_position_uniform,
                1,
                CAMERA_POSITION.as_ptr(),
            );
            gl::Uniform3fv(
                vertex_array.check_program.light_position_uniform,
                1,
                LIGHT_POSITION.as_ptr(),
            );
            gl::Uniform2f(
                vertex_array.check_program.sphere_position_uniform,
                ball_rect.center().x,
                ball_rect.center().y,
            );
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            ck();
        }

        let old_surface = device.unbind_surface_from_context(&mut context).unwrap();
        let new_surface = match worker_from_main_receiver.recv() {
            Ok(surface) => surface,
            Err(_) => break,
        };

        device
            .bind_surface_to_context(&mut context, new_surface)
            .unwrap();
        worker_to_main_sender
            .send(Frame {
                surface: old_surface,
                viewport_origin: ball_rect.origin - subscreen_offset,
                sphere_position: ball_rect.center(),
            })
            .unwrap();

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
        if ball_rect.max_x() >= window_size.width as f32 {
            ball_rect.origin.x = (window_size.width - BALL_WIDTH) as f32;
            ball_velocity.x = -f32::abs(ball_velocity.x);
        }

        // Rotate.
        theta_x += ROTATION_SPEED_X;
        theta_y += ROTATION_SPEED_Y;
        theta_z += ROTATION_SPEED_Z;
    }

    device.destroy_context(&mut context).unwrap();
}

struct Frame {
    surface: Option<Surface>,
    viewport_origin: Point2D<f32>,
    sphere_position: Point2D<f32>,
}

struct BlitVertexArray {
    object: GLuint,
    blit_program: BlitProgram,
    #[allow(dead_code)]
    position_buffer: Buffer,
}

impl BlitVertexArray {
    fn new(
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> BlitVertexArray {
        let blit_program = BlitProgram::new(gl_api, gl_texture_target, resource_loader);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array);
            ck();
            gl::BindVertexArray(vertex_array);
            ck();

            let position_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, position_buffer.object);
            ck();
            gl::VertexAttribPointer(
                blit_program.position_attribute as GLuint,
                2,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                2,
                0 as *const GLvoid,
            );
            ck();
            gl::EnableVertexAttribArray(blit_program.position_attribute as GLuint);
            ck();

            BlitVertexArray {
                object: vertex_array,
                blit_program,
                position_buffer,
            }
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
    fn new(
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> GridVertexArray {
        let grid_program = GridProgram::new(gl_api, gl_texture_target, resource_loader);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array);
            ck();
            gl::BindVertexArray(vertex_array);
            ck();

            let position_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, position_buffer.object);
            ck();
            gl::VertexAttribPointer(
                grid_program.position_attribute as GLuint,
                2,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                2,
                0 as *const GLvoid,
            );
            ck();
            gl::EnableVertexAttribArray(grid_program.position_attribute as GLuint);
            ck();

            GridVertexArray {
                object: vertex_array,
                grid_program,
                position_buffer,
            }
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
    fn new(
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> CheckVertexArray {
        let check_program = CheckProgram::new(gl_api, gl_texture_target, resource_loader);
        unsafe {
            let mut vertex_array = 0;
            gl::GenVertexArrays(1, &mut vertex_array);
            ck();
            gl::BindVertexArray(vertex_array);
            ck();

            let position_buffer = Buffer::from_data(&QUAD_VERTEX_POSITIONS);
            gl::BindBuffer(gl::ARRAY_BUFFER, position_buffer.object);
            ck();
            gl::VertexAttribPointer(
                check_program.position_attribute as GLuint,
                2,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                2,
                0 as *const GLvoid,
            );
            ck();
            gl::EnableVertexAttribArray(check_program.position_attribute as GLuint);
            ck();

            CheckVertexArray {
                object: vertex_array,
                check_program,
                position_buffer,
            }
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
    fn new(
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> BlitProgram {
        let vertex_shader = Shader::new(
            "quad",
            ShaderKind::Vertex,
            gl_api,
            gl_texture_target,
            resource_loader,
        );
        let fragment_shader = Shader::new(
            "blit",
            ShaderKind::Fragment,
            gl_api,
            gl_texture_target,
            resource_loader,
        );
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object, b"aPosition\0".as_ptr() as *const GLchar);
            ck();
            let transform_uniform =
                gl::GetUniformLocation(program.object, b"uTransform\0".as_ptr() as *const GLchar);
            ck();
            let translation_uniform =
                gl::GetUniformLocation(program.object, b"uTranslation\0".as_ptr() as *const GLchar);
            ck();
            let tex_transform_uniform = gl::GetUniformLocation(
                program.object,
                b"uTexTransform\0".as_ptr() as *const GLchar,
            );
            ck();
            let tex_translation_uniform = gl::GetUniformLocation(
                program.object,
                b"uTexTranslation\0".as_ptr() as *const GLchar,
            );
            ck();
            let source_uniform =
                gl::GetUniformLocation(program.object, b"uSource\0".as_ptr() as *const GLchar);
            ck();
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
    radius_uniform: GLint,
    sphere_position_uniform: GLint,
    camera_position_uniform: GLint,
    light_position_uniform: GLint,
}

impl GridProgram {
    fn new(
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> GridProgram {
        let vertex_shader = Shader::new(
            "quad",
            ShaderKind::Vertex,
            gl_api,
            gl_texture_target,
            resource_loader,
        );
        let fragment_shader = Shader::new(
            "grid",
            ShaderKind::Fragment,
            gl_api,
            gl_texture_target,
            resource_loader,
        );
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object, b"aPosition\0".as_ptr() as *const GLchar);
            ck();
            let transform_uniform =
                gl::GetUniformLocation(program.object, b"uTransform\0".as_ptr() as *const GLchar);
            ck();
            let translation_uniform =
                gl::GetUniformLocation(program.object, b"uTranslation\0".as_ptr() as *const GLchar);
            ck();
            let tex_transform_uniform = gl::GetUniformLocation(
                program.object,
                b"uTexTransform\0".as_ptr() as *const GLchar,
            );
            ck();
            let tex_translation_uniform = gl::GetUniformLocation(
                program.object,
                b"uTexTranslation\0".as_ptr() as *const GLchar,
            );
            ck();
            let gridline_color_uniform = gl::GetUniformLocation(
                program.object,
                b"uGridlineColor\0".as_ptr() as *const GLchar,
            );
            ck();
            let bg_color_uniform =
                gl::GetUniformLocation(program.object, b"uBGColor\0".as_ptr() as *const GLchar);
            ck();
            let radius_uniform =
                gl::GetUniformLocation(program.object, b"uRadius\0".as_ptr() as *const GLchar);
            ck();
            let camera_position_uniform = gl::GetUniformLocation(
                program.object,
                b"uCameraPosition\0".as_ptr() as *const GLchar,
            );
            ck();
            let light_position_uniform = gl::GetUniformLocation(
                program.object,
                b"uLightPosition\0".as_ptr() as *const GLchar,
            );
            ck();
            let sphere_position_uniform = gl::GetUniformLocation(
                program.object,
                b"uSpherePosition\0".as_ptr() as *const GLchar,
            );
            ck();
            GridProgram {
                program,
                position_attribute,
                transform_uniform,
                translation_uniform,
                tex_transform_uniform,
                tex_translation_uniform,
                gridline_color_uniform,
                bg_color_uniform,
                radius_uniform,
                camera_position_uniform,
                light_position_uniform,
                sphere_position_uniform,
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
    fn new(
        gl_api: GLApi,
        gl_texture_target: GLenum,
        resource_loader: &dyn ResourceLoader,
    ) -> CheckProgram {
        let vertex_shader = Shader::new(
            "quad",
            ShaderKind::Vertex,
            gl_api,
            gl_texture_target,
            resource_loader,
        );
        let fragment_shader = Shader::new(
            "check",
            ShaderKind::Fragment,
            gl_api,
            gl_texture_target,
            resource_loader,
        );
        let program = Program::new(vertex_shader, fragment_shader);
        unsafe {
            let position_attribute =
                gl::GetAttribLocation(program.object, b"aPosition\0".as_ptr() as *const GLchar);
            ck();
            let transform_uniform =
                gl::GetUniformLocation(program.object, b"uTransform\0".as_ptr() as *const GLchar);
            ck();
            let translation_uniform =
                gl::GetUniformLocation(program.object, b"uTranslation\0".as_ptr() as *const GLchar);
            ck();
            let tex_transform_uniform = gl::GetUniformLocation(
                program.object,
                b"uTexTransform\0".as_ptr() as *const GLchar,
            );
            ck();
            let tex_translation_uniform = gl::GetUniformLocation(
                program.object,
                b"uTexTranslation\0".as_ptr() as *const GLchar,
            );
            ck();
            let rotation_uniform =
                gl::GetUniformLocation(program.object, b"uRotation\0".as_ptr() as *const GLchar);
            ck();
            let color_a_uniform =
                gl::GetUniformLocation(program.object, b"uColorA\0".as_ptr() as *const GLchar);
            ck();
            let color_b_uniform =
                gl::GetUniformLocation(program.object, b"uColorB\0".as_ptr() as *const GLchar);
            ck();
            let viewport_origin_uniform = gl::GetUniformLocation(
                program.object,
                b"uViewportOrigin\0".as_ptr() as *const GLchar,
            );
            ck();
            let radius_uniform =
                gl::GetUniformLocation(program.object, b"uRadius\0".as_ptr() as *const GLchar);
            ck();
            let camera_position_uniform = gl::GetUniformLocation(
                program.object,
                b"uCameraPosition\0".as_ptr() as *const GLchar,
            );
            ck();
            let light_position_uniform = gl::GetUniformLocation(
                program.object,
                b"uLightPosition\0".as_ptr() as *const GLchar,
            );
            ck();
            let sphere_position_uniform = gl::GetUniformLocation(
                program.object,
                b"uSpherePosition\0".as_ptr() as *const GLchar,
            );
            ck();
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
