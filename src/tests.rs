use gleam::gl;
use geom::Size2D;

use GLContext;
use GLContextAttributes;
use ColorAttachmentType;

use std::ffi::CString;

#[cfg(target_os = "linux")]
use glx;
#[cfg(target_os = "android")]
use egl;

#[cfg(target_os = "macos")]
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;

use std::str::FromStr;

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

static mut GL_LOADED: bool = false;

// Shamelessly stolen from glutin
#[cfg(target_os = "linux")]
fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            glx::GetProcAddress(addr as *const _) as *const ()
        }
}

#[cfg(target_os = "android")]
fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(s.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            egl::GetProcAddress(addr as *const _) as *const ()
        }
}

#[cfg(target_os = "macos")]
fn get_proc_address(addr: &str) -> *const () {
        let symbol_name: CFString = FromStr::from_str(addr).unwrap();
        let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
        let framework = unsafe {
            CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef())
        };
        let symbol = unsafe {
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef())
        };
        symbol as *const ()
}

fn load_gl() {
    unsafe {
        if GL_LOADED {
            return;
        }

        gl::load_with(|s| get_proc_address(s) as *const _);
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

#[test]
#[cfg(feature="texture_surface")]
fn test_texture_surface_color_attachment() {
    load_gl();
    let size : Size2D<i32> = Size2D(256, 256);
    let ctx = GLContext::create_offscreen_with_color_attachment(size, GLContextAttributes::default(), ColorAttachmentType::TextureWithSurface).unwrap();
    test_gl_context(&ctx);


    let surface = ctx.borrow_draw_buffer().unwrap().borrow_bound_surface().unwrap();
    let (flip, target) = Texture::texture_flip_and_target(true);
    let mut texture = Texture::new(target, Size2D(size.width as usize, size.height as usize));
    texture.flip = flip;

    surface.bind_to_texture(&NativeCompositingGraphicsContext::new(), &texture, Size2D(size.width as isize, size.height as isize));

    ctx.make_current().unwrap();
    unsafe {
        gl::ClearColor(1.0, 1.0, 1.0, 1.0); // This should be overriden by the drawing of the texture
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    // test_gl_context_draw_buffer_is_red(&ctx);

    let _bound = texture.bind();

    let mut vec : Vec<u8> = vec![];

    vec.reserve((size.width * size.height * 4) as usize);
    unsafe {
        gl::TexImage2D(texture.target.as_gl_target(), 0, gl::RGBA8 as i32, size.width, size.height, 0, gl::RGBA, gl::UNSIGNED_BYTE, vec.as_mut_ptr() as *mut _);
        vec.set_len((size.width * size.height * 4) as usize);
    }

    test_pixels(&vec);
}
