use gleam::gl;
use geom::Size2D;

use GLContext;
use GLContextAttributes;
use ColorAttachmentType;

#[cfg(feature="texture_surface")]
use layers::texturegl::Texture;

#[cfg(feature="texture_surface")]
use layers::platform::surface::NativeCompositingGraphicsContext;

#[cfg(target_os="macos")]
#[link(name="OpenGL", kind="framework")]
extern {}

#[cfg(target_os="linux")]
#[link(name="GL")]
extern {}

// This is probably a time bomb
static mut GL_LOADED : bool = false;

fn load_gl() {
    unsafe {
        if GL_LOADED {
            return;
        }

        gl::load_with(|s| GLContext::get_proc_address(s) as *const _);
        GL_LOADED = true;
    }
}

fn test_gl_context(context: &GLContext) {
    context.make_current().unwrap();

    unsafe {
        gl::ClearColor(1.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let size = context.draw_buffer_size().unwrap();

    let pixels = gl::read_pixels(0, 0, size.width, size.height, gl::RGBA, gl::UNSIGNED_BYTE);

    assert!(pixels.len() == (size.width * size.height * 4) as usize);
    test_pixels(&pixels);
}

fn test_pixels(pixels: &Vec<u8>) {
    for pixel in pixels.chunks(4) {
        println!("{:?}", pixel);
        assert!(pixel[0] == 255);
        assert!(pixel[1] == 0);
        assert!(pixel[2] == 0);
        assert!(pixel[3] == 255);
    }
}

#[test]
fn test_default_color_attachment() {
    load_gl();
    test_gl_context(&GLContext::create_offscreen(Size2D(256, 256), GLContextAttributes::default()).unwrap());
}

#[test]
fn test_texture_color_attachment() {
    load_gl();
    test_gl_context(&GLContext::create_offscreen_with_color_attachment(Size2D(256, 256), GLContextAttributes::default(), ColorAttachmentType::Texture).unwrap())
}

#[cfg(target_os="linux")]
#[cfg(feature="texture_surface")]
fn get_compositing_context(gl_context: &GLContext) -> NativeCompositingGraphicsContext {
    NativeCompositingGraphicsContext::from_display(gl_context.get_metadata().display)
}

#[cfg(not(target_os="linux"))]
#[cfg(feature="texture_surface")]
fn get_compositing_context(_: &GLContext) -> NativeCompositingGraphicsContext {
    NativeCompositingGraphicsContext::new()
}


#[test]
#[cfg(feature="texture_surface")]
fn test_texture_surface_color_attachment() {
    load_gl();
    let size : Size2D<i32> = Size2D(256, 256);
    let ctx = GLContext::create_offscreen_with_color_attachment(size, GLContextAttributes::default(), ColorAttachmentType::TextureWithSurface).unwrap();

    test_gl_context(&ctx);

    // Pick up the (in theory) painted surface
    // And bind it to a new Texture
    let surface = ctx.borrow_draw_buffer().unwrap().borrow_bound_surface().unwrap();
    let (flip, target) = Texture::texture_flip_and_target(true);
    let mut texture = Texture::new(target, Size2D(size.width as usize, size.height as usize));
    texture.flip = flip;

    let compositing_context = get_compositing_context(&ctx);

    surface.bind_to_texture(&compositing_context, &texture, Size2D(size.width as isize, size.height as isize));

    // Bind the texture, get its pixels in rgba format and test
    // if it has the surface contents
    let _bound = texture.bind();

    let mut vec : Vec<u8> = vec![];

    vec.reserve((size.width * size.height * 4) as usize);
    unsafe {
        gl::TexImage2D(texture.target.as_gl_target(), 0, gl::RGBA8 as i32, size.width, size.height, 0, gl::RGBA, gl::UNSIGNED_BYTE, vec.as_mut_ptr() as *mut _);
        vec.set_len((size.width * size.height * 4) as usize);
    }

    test_pixels(&vec);
}
