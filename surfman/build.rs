// surfman/surfman/build.rs
//
//! The `surfman` build script.

use cfg_aliases::cfg_aliases;
use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};
use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    // Setup aliases for #[cfg] checks
    cfg_aliases! {
        // Platforms
        windows: { target_os = "windows" },
        macos: { target_os = "macos" },
        android: { target_os = "android" },
        // TODO: is `target_os = "linux"` the same as the following check?
        linux: { all(unix, not(any(macos, android))) },

        // Features:
        // Here we collect the features that are only valid on certain platforms and
        // we add aliases that include checks for the correct platform.
        angle: { all(windows, feature = "sm-angle") },
        angle_builtin: { all(windows, feature = "sm-angle-builtin") },
        angle_default: { all(windows, feature = "sm-angle-default") },
        no_wgl: { all(windows, feature = "sm-no-wgl") },
        wayland_default: { all(linux, feature = "sm-wayland-default") },
        x11: { all(linux, feature = "sm-x11") },
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").ok();
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    // Generate EGL bindings.
    if target_os == "android"
        || (target_os == "windows" && cfg!(feature = "sm-angle"))
        || target_family.as_ref().map_or(false, |f| f == "unix")
    {
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
