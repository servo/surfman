use GLContextAttributes;
use GLContext;

use geom::Size2D;
use std::mem;
use std::iter::range_step;
use std::ffi::{CStr};
use gleam::gl;
use gleam::gl::types::*;
use glutin;

static mut gl_loaded : bool = false;

fn load_gl() {
    unsafe {
        if gl_loaded {
            return;
        }

        let window = glutin::Window::new().unwrap();

        window.make_current();

        gl::load_with(|s| window.get_proc_address(s));

        gl_loaded = true;
    }
}

#[test]
fn gl_context_works() {
    load_gl();

    let size = Size2D(16, 16);
    let context = GLContext::create_offscreen(size, GLContextAttributes::default()).unwrap();

    context.make_current().unwrap();

    // ClearColor with a green background
    unsafe {
        gl::ClearColor(0.0, 1.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }


    let pixels = gl::read_pixels(0, 0, size.height, size.width, gl::RGBA, gl::UNSIGNED_BYTE);

    println!("read {} pixel bytes", pixels.len());
    assert!(pixels.len() == (size.height * size.width * 4) as usize);

    for i in range_step(0, pixels.len(), 4) {
        println!("{} {} {} {}", pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]);
        // assert_eq!(pixels[i], 0);
        // assert_eq!(pixels[i + 1], 255);
        // assert_eq!(pixels[i + 2], 0);
        // assert_eq!(pixels[i + 3], 255);
    }

    assert!(false);
}
