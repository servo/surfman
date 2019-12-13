// surfman/surfman/build.rs
//
//! The `surfman` build script.

use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};
use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    // Generate EGL bindings.
    if target_os == "android" ||
            (target_os == "windows" && cfg!(feature = "sm-angle")) ||
            target_family == "unix" {
        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        let registry = Registry::new(Api::Egl, (1, 5), Profile::Core, Fallbacks::All, []);
        registry.write_bindings(StructGenerator, &mut file).unwrap();
    }

    // Generate GL bindings.
    if target_os == "android" {
        let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();
        let registry = Registry::new(Api::Gles2, (3, 0), Profile::Core, Fallbacks::All, []);
        registry.write_bindings(StructGenerator, &mut file).unwrap();
    } else {
        let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();
        let registry = Registry::new(Api::Gl, (3, 3), Profile::Core, Fallbacks::All, []);
        registry.write_bindings(StructGenerator, &mut file).unwrap();
    }
}
