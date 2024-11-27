// surfman/surfman/examples/chaos_game.rs
//
//! Demonstrates how to use `surfman` to draw to a window surface via the CPU.

use euclid::default::{Point2D, Size2D};
use rand::{self, Rng};
use surfman::{SurfaceAccess, SurfaceType};
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent::KeyboardInput;
use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, RawKeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle, HasWindowHandle};
use winit::window::WindowBuilder;

#[cfg(target_os = "macos")]
use surfman::SystemConnection;

const WINDOW_WIDTH: i32 = 800;
const WINDOW_HEIGHT: i32 = 600;

const BYTES_PER_PIXEL: usize = 4;

const FOREGROUND_COLOR: u32 = !0;

const ITERATIONS_PER_FRAME: usize = 20;

static TRIANGLE_POINTS: [(f32, f32); 3] = [
    (400.0, 300.0 + 75.0 + 150.0),
    (400.0 + 259.81, 300.0 + 75.0 - 300.0),
    (400.0 - 259.81, 300.0 + 75.0 - 300.0),
];

#[cfg(not(all(target_os = "macos", feature = "sm-raw-window-handle-06")))]
fn main() {
    println!("The `chaos_game` demo is not yet supported on this platform.");
}

#[cfg(all(target_os = "macos", feature = "sm-raw-window-handle-06"))]
fn main() {
    let connection = SystemConnection::new().unwrap();
    let adapter = connection.create_adapter().unwrap();
    let mut device = connection.create_device(&adapter).unwrap();

    let event_loop = EventLoop::new().unwrap();
    let physical_size = PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let window = WindowBuilder::new()
        .with_title("Chaos game example")
        .with_inner_size(physical_size)
        .build(&event_loop)
        .unwrap();

    window.set_visible(true);

    let window_size = window.inner_size();
    let window_size = Size2D::new(window_size.width as i32, window_size.height as i32);
    let handle = window.window_handle().unwrap();
    let native_widget = connection
        .create_native_widget_from_window_handle(handle, window_size)
        .unwrap();

    let surface_type = SurfaceType::Widget { native_widget };
    let mut surface = device
        .create_surface(SurfaceAccess::GPUCPU, surface_type)
        .unwrap();

    let mut rng = rand::thread_rng();
    let mut point = Point2D::new(WINDOW_WIDTH as f32 * 0.5, WINDOW_HEIGHT as f32 * 0.5);
    let mut data = vec![0; WINDOW_WIDTH as usize * WINDOW_HEIGHT as usize * 4];

    event_loop.run(move |event, event_loop| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                logical_key: Key::Named(NamedKey::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => event_loop.exit(),
            _ => {
                for _ in 0..ITERATIONS_PER_FRAME {
                    let (dest_x, dest_y) = TRIANGLE_POINTS[rng.gen_range(0..3)];
                    point = point.lerp(Point2D::new(dest_x, dest_y), 0.5);
                    put_pixel(&mut data, &point, FOREGROUND_COLOR);
                }

                device
                    .lock_surface_data(&mut surface)
                    .unwrap()
                    .data()
                    .copy_from_slice(&data);
                device.present_surface(&mut surface).unwrap();
            }
        };
    });
}

fn put_pixel(data: &mut [u8], point: &Point2D<f32>, color: u32) {
    let (x, y) = (f32::round(point.x) as usize, f32::round(point.y) as usize);
    let start = (y * WINDOW_WIDTH as usize + x) * BYTES_PER_PIXEL;
    for index in 0..BYTES_PER_PIXEL {
        data[index + start] = (color >> (index * 8)) as u8;
    }
}
