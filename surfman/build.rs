// surfman/build.rs

use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};
use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    if (target_os == "android")
        || ((target_os == "windows") && cfg!(feature = "sm-angle"))
        || (target_family == "unix")
        || cfg!(feature = "test_egl_in_linux")
    {
        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        let registry = Registry::new(Api::Egl, (1, 5), Profile::Core, Fallbacks::All, []);
        registry.write_bindings(StructGenerator, &mut file).unwrap();
    }

    if cfg!(feature = "sm-x11")
        || ((target_family == "unix") && (target_os != "macos") && (target_os != "android"))
    {
        let mut file = File::create(&dest.join("glx_bindings.rs")).unwrap();
        Registry::new(Api::Glx, (1, 4), Profile::Core, Fallbacks::All, [
            "GLX_ARB_create_context",
            "GLX_EXT_texture_from_pixmap",
        ]).write_bindings(StructGenerator, &mut file).unwrap();
        println!("cargo:rustc-link-lib=GL");
    }

    let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();
    let registry = Registry::new(Api::Gl, (3, 3), Profile::Core, Fallbacks::All, []);
    registry.write_bindings(StructGenerator, &mut file).unwrap();
}
