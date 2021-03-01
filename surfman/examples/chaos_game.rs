// surfman/surfman/examples/chaos_game.rs
//
//! Demonstrates how to use `surfman` to draw to a window surface via the CPU.

use euclid::default::Point2D;
use rand::{self, Rng};
use surfman::{SurfaceAccess, SurfaceType};
use winit::dpi::PhysicalSize;
use winit::{DeviceEvent, Event, EventsLoop, KeyboardInput, VirtualKeyCode};
use winit::{WindowBuilder, WindowEvent};

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

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("The `chaos_game` demo is not yet supported on this platform.");
}

#[cfg(target_os = "macos")]
fn main() {
    let connection = SystemConnection::new().unwrap();
    let adapter = connection.create_adapter().unwrap();
    let mut device = connection.create_device(&adapter).unwrap();

    let mut event_loop = EventsLoop::new();
    let dpi = event_loop.get_primary_monitor().get_scale_factor();
    let logical_size = PhysicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64).to_logical(dpi);
    let window = WindowBuilder::new()
        .with_title("Chaos game example")
        .with_dimensions(logical_size)
        .build(&event_loop)
        .unwrap();
    window.show();

    let native_widget = connection
        .create_native_widget_from_winit_window(&window)
        .unwrap();

    let surface_type = SurfaceType::Widget { native_widget };
    let mut surface = device
        .create_surface(SurfaceAccess::GPUCPU, surface_type)
        .unwrap();

    let mut rng = rand::thread_rng();
    let mut point = Point2D::new(WINDOW_WIDTH as f32 * 0.5, WINDOW_HEIGHT as f32 * 0.5);
    let mut data = vec![0; WINDOW_WIDTH as usize * WINDOW_HEIGHT as usize * 4];

    let mut exit = false;
    while !exit {
        for _ in 0..ITERATIONS_PER_FRAME {
            let (dest_x, dest_y) = TRIANGLE_POINTS[rng.gen_range(0, 3)];
            point = point.lerp(Point2D::new(dest_x, dest_y), 0.5);
            put_pixel(&mut data, &point, FOREGROUND_COLOR);
        }

        device
            .lock_surface_data(&mut surface)
            .unwrap()
            .data()
            .copy_from_slice(&data);
        device.present_surface(&mut surface).unwrap();

        event_loop.poll_events(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            }
            | Event::DeviceEvent {
                event:
                    DeviceEvent::Key(KeyboardInput {
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    }),
                ..
            } => exit = true,
            _ => {}
        });
    }
}

fn put_pixel(data: &mut [u8], point: &Point2D<f32>, color: u32) {
    let (x, y) = (f32::round(point.x) as usize, f32::round(point.y) as usize);
    let start = (y * WINDOW_WIDTH as usize + x) * BYTES_PER_PIXEL;
    for index in 0..BYTES_PER_PIXEL {
        data[index + start] = (color >> (index * 8)) as u8;
    }
}
