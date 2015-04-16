use common_methods::GLContextMethods;
use gl_context_attributes::GLContextAttributes;
use platform::GLContext;
use geom::Size2D;
use gleam::{self, gl};
use std::iter::range_step;

#[test]
fn gl_context_works() {
    let context = GLContext::create_offscreen(Size2D(4, 4), GLContextAttributes::default()).unwrap();

    context.make_current().unwrap();

    // ClearColor with a green background
    unsafe {
        gl::ClearColor(0.0, 1.0, 0.0, 1.0);
    }

    let pixels = gl::read_pixels(0, 0, 4, 4, gl::RGBA, gl::UNSIGNED_BYTE);

    for i in range_step(0, pixels.len(), 4) {
        assert_eq!(pixels[i], 0);
        assert_eq!(pixels[i + 1], 255);
        assert_eq!(pixels[i + 2], 0);
        assert_eq!(pixels[i + 3], 255);
    }
}
