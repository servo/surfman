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
        android_platform: { target_os = "android" },
        ohos_platform: { target_env = "ohos" },
        web_platform: { all(target_family = "wasm", target_os = "unknown") },
        macos_platform: { target_os = "macos" },
        ios_platform: { target_os = "ios" },
        windows_platform: { target_os = "windows" },
        apple: { any(target_os = "ios", target_os = "macos") },
        free_unix: { all(unix, not(apple), not(android_platform), not(target_os = "emscripten"), not(ohos_platform)) },

        // Native displays.
        x11_platform: { all(free_unix, feature = "sm-x11") },
        wayland_platform: { all(free_unix) },

        // Features:
        // Here we collect the features that are only valid on certain platforms and
        // we add aliases that include checks for the correct platform.
        angle: { all(windows, feature = "sm-angle") },
        angle_builtin: { all(windows_platform, feature = "sm-angle-builtin") },
        angle_default: { all(windows_platform, feature = "sm-angle-default") },
        no_wgl: { all(windows_platform, feature = "sm-no-wgl") },
        wayland_default: { all(wayland_platform, any(not(x11_platform), feature = "sm-wayland-default")) },
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").ok();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    // Generate EGL bindings.
    if target_os == "android"
        || (target_os == "windows" && cfg!(feature = "sm-angle"))
        || target_env == "ohos"
        || target_family.as_ref().map_or(false, |f| f == "unix")
    {
        let mut file = File::create(dest.join("egl_bindings.rs")).unwrap();
        let registry = Registry::new(Api::Egl, (1, 5), Profile::Core, Fallbacks::All, []);
        registry.write_bindings(StructGenerator, &mut file).unwrap();
    }
}
