use gleam::gl;
use geom::Size2D;
use std::mem;
use GLContext;
use GLContextAttributes;
use std::ffi::CString;

#[cfg(target_os = "linux")]
use glx;
#[cfg(target_os = "android")]
use egl;

#[link(name="GL")]
extern {}

static mut GL_LOADED: bool = false;

// Shamelessly stolen from glutin
#[cfg(target_os = "linux")]
fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(s.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        glx::GetProcAddress(addr as *const _)
}

#[cfg(target_os = "android")]
fn get_proc_address(addr: &str) -> *const () {
        let addr = CString::new(s.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        egl::GetProcAddress(addr as *const _)
}

fn load_gl() {
    unsafe {
        if GL_LOADED {
            return;
        }

        gl::load_with(|s| {
        });
        GL_LOADED = true;
    }
}

#[test]
fn test_offscreen() {
    load_gl();
    let context = GLContext::create_offscreen(Size2D(256, 256), GLContextAttributes::default()).unwrap();

    context.make_current().unwrap();

    unsafe {
        gl::ClearColor(1.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let pixel = gl::read_pixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE);

    println!("{} {} {} {}", pixel[0], pixel[1], pixel[2], pixel[3]);

    assert!(pixel[0] == 255);
    assert!(pixel[1] == 0);
    assert!(pixel[2] == 0);
    assert!(pixel[3] == 255);
}
