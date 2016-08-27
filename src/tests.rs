use gleam::gl;
use gleam::gl::types::GLint;
use euclid::Size2D;
use std::sync::{Once, ONCE_INIT};

use GLContext;
#[cfg(all(target_os = "linux", feature = "test_egl_in_linux"))]
use platform::with_egl::NativeGLContext;
#[cfg(not(all(target_os = "linux", feature = "test_egl_in_linux")))]
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

fn test_pixels_eq(pixels: &[u8], to: &[u8]) {
    assert!(to.len() == 4);
    for pixel in pixels.chunks(4) {
        assert_eq!(pixel, to);
    }

}

fn test_pixels(pixels: &[u8]) {
    test_pixels_eq(pixels, &[255, 0, 0, 255]);
}


#[test]
fn test_unbinding() {
    load_gl();
    let ctx = GLContext::<NativeGLContext>::new(Size2D::new(256, 256),
                                                GLContextAttributes::default(),
                                                ColorAttachmentType::Renderbuffer,
                                                None).unwrap();

    assert!(NativeGLContext::current_handle().is_some());

    ctx.unbind().unwrap();
    assert!(NativeGLContext::current_handle().is_none());
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

    assert!(gl::get_error() == gl::NO_ERROR);

    // Actually we just check that writing to the framebuffer works, and that there's a texture
    // attached to it. Doing a getTexImage should be a good idea, but it's not available on gles,
    // so what we should do is rebinding to another FBO.
    //
    // This is done in the `test_sharing` test though, so if that passes we know everything
    // works and we're just happy.
    let vec = gl::read_pixels(0, 0, size.width, size.height, gl::RGBA, gl::UNSIGNED_BYTE);
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

    // Ensure the old texture is bound, and bind the new one
    assert!(gl::get_integer_v(gl::TEXTURE_BINDING_2D) == primary_texture_id as GLint);


    // Clearing and re-binding to a framebuffer instead of using getTexImage since it's not
    // available in GLES2
    gl::clear_color(0.0, 0.0, 0.0, 1.0);
    gl::clear(gl::COLOR_BUFFER_BIT);

    let vec = gl::read_pixels(0, 0, size.width, size.height, gl::RGBA, gl::UNSIGNED_BYTE);
    test_pixels_eq(&vec, &[0, 0, 0, 255]);

    gl::bind_texture(gl::TEXTURE_2D, secondary_texture_id);

    unsafe {
        gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                 gl::COLOR_ATTACHMENT0,
                                 gl::TEXTURE_2D,
                                 secondary_texture_id, 0);
    }

    let vec = gl::read_pixels(0, 0, size.width, size.height, gl::RGBA, gl::UNSIGNED_BYTE);
    assert!(gl::get_error() == gl::NO_ERROR);

    test_pixels(&vec);
}

#[test]
fn test_limits() {
    load_gl();

    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    GLContextAttributes::default(),
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();
    assert!(context.borrow_limits().max_vertex_attribs != 0);
}

#[test]
fn test_no_alpha() {
    load_gl();
    let mut attributes = GLContextAttributes::default();
    attributes.alpha = false;

    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    attributes,
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();
    assert!(context.borrow_limits().max_vertex_attribs != 0);
}

#[test]
fn test_no_depth() {
    load_gl();
    let mut attributes = GLContextAttributes::default();
    attributes.depth = false;

    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    attributes,
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();
    assert!(context.borrow_limits().max_vertex_attribs != 0);
}

#[test]
fn test_no_depth_no_alpha() {
    load_gl();
    let mut attributes = GLContextAttributes::default();
    attributes.depth = false;
    attributes.alpha = false;

    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    attributes,
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();
    assert!(context.borrow_limits().max_vertex_attribs != 0);
}

#[test]
fn test_no_premul_alpha() {
    load_gl();
    let mut attributes = GLContextAttributes::default();
    attributes.depth = false;
    attributes.alpha = false;
    attributes.premultiplied_alpha = false;

    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    attributes,
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();
    assert!(context.borrow_limits().max_vertex_attribs != 0);
}

#[test]
fn test_in_a_row() {
    load_gl();
    let mut attributes = GLContextAttributes::default();
    attributes.depth = false;
    attributes.alpha = false;
    attributes.premultiplied_alpha = false;

    let size = Size2D::new(256, 256);
    let context = GLContext::<NativeGLContext>::new(size,
                                                    attributes.clone(),
                                                    ColorAttachmentType::Texture,
                                                    None).unwrap();

    let handle = context.handle();

    GLContext::<NativeGLContext>::new(size,
                                      attributes.clone(),
                                      ColorAttachmentType::Texture,
                                      Some(&handle)).unwrap();

    GLContext::<NativeGLContext>::new(size,
                                      attributes.clone(),
                                      ColorAttachmentType::Texture,
                                      Some(&handle)).unwrap();
}
