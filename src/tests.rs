use gleam::gl;
use gleam::gl::types::GLint;
use euclid::Size2D;
use std::sync::{Once, ONCE_INIT};

use GLContext;
use NativeGLContext;
use NativeGLContextMethods;
use GLContextAttributes;
use ColorAttachmentType;

#[cfg(target_os="macos")]
#[link(name="OpenGL", kind="framework")]
extern {}

#[cfg(target_os="linux")]
#[link(name="GL")]
extern {}

static LOAD_GL: Once = ONCE_INIT;


fn load_gl() {
    LOAD_GL.call_once(|| {
        gl::load_with(|s| GLContext::<NativeGLContext>::get_proc_address(s) as *const _);
    });
}

fn test_gl_context<T: NativeGLContextMethods>(context: &GLContext<T>) {
    context.make_current().unwrap();

    gl::clear_color(1.0, 0.0, 0.0, 1.0);
    gl::clear(gl::COLOR_BUFFER_BIT);

    let size = context.draw_buffer_size().unwrap();

    let pixels = gl::read_pixels(0, 0, size.width, size.height, gl::RGBA, gl::UNSIGNED_BYTE);

    assert!(pixels.len() == (size.width * size.height * 4) as usize);
    test_pixels(&pixels);
}

fn test_pixels(pixels: &[u8]) {
    let mut idx = 0;
    for pixel in pixels.chunks(4) {
        println!("{}: {:?}", idx, pixel);
        assert!(pixel[0] == 255);
        assert!(pixel[1] == 0);
        assert!(pixel[2] == 0);
        assert!(pixel[3] == 255);
        idx += 1;
    }
}

#[test]
fn test_renderbuffer_color_attachment() {
    load_gl();
    test_gl_context(&GLContext::<NativeGLContext>::new(Size2D::new(256, 256),
                                                       GLContextAttributes::default(),
                                                       ColorAttachmentType::Renderbuffer,
                                                       None).unwrap());
}

#[test]
fn test_texture_color_attachment() {
    load_gl();
    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    GLContextAttributes::default(),
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();
    test_gl_context(&context);


    // Get the bound texture and check we're painting on it
    let texture_id = context.borrow_draw_buffer().unwrap().get_bound_texture_id().unwrap();
    assert!(texture_id != 0);

    let mut vec = vec![0u8; (size.width * size.height * 4) as usize];
    unsafe {
        gl::GetTexImage(gl::TEXTURE_2D, 0, gl::RGBA as u32, gl::UNSIGNED_BYTE, vec.as_mut_ptr() as *mut _);
    }
    assert!(gl::get_error() == gl::NO_ERROR);

    test_pixels(&vec);
}

#[test]
fn test_sharing() {
    load_gl();

    let size = Size2D::new(256, 256);
    let primary = GLContext::<NativeGLContext>::new(size,
                                                    GLContextAttributes::default(),
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();

    let primary_texture_id = primary.borrow_draw_buffer().unwrap().get_bound_texture_id().unwrap();
    assert!(primary_texture_id != 0);

    let secondary = GLContext::<NativeGLContext>::new(size,
                                                      GLContextAttributes::default(),
                                                      ColorAttachmentType::Texture,
                                                      Some(&primary.handle())).unwrap();

    // Paint the second context red
    test_gl_context(&secondary);

    // Now the secondary context is bound, get the texture id, switch contexts, and check the
    // texture is there.
    let secondary_texture_id = secondary.borrow_draw_buffer().unwrap().get_bound_texture_id().unwrap();
    assert!(secondary_texture_id != 0);

    primary.make_current().unwrap();
    assert!(unsafe { gl::IsTexture(secondary_texture_id) != 0 });

    let mut vec = vec![0u8; (size.width * size.height * 4) as usize];

    // Ensure the old texture is bound, and bind the new one
    assert!(gl::get_integer_v(gl::TEXTURE_BINDING_2D) == primary_texture_id as GLint);

    gl::bind_texture(gl::TEXTURE_2D, secondary_texture_id);
    unsafe {
        gl::GetTexImage(gl::TEXTURE_2D, 0, gl::RGBA as u32, gl::UNSIGNED_BYTE, vec.as_mut_ptr() as *mut _);
    }

    assert!(gl::get_error() == gl::NO_ERROR);

    test_pixels(&vec);
}
