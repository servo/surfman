// surfman/surfman/src/macros.rs
//
//! A macro for use in the top-level crate.

/// When using `surfman`, you should place this macro at the top of your crate, like so:
///
/// ```ignore
/// use surfman::macros::declare_surfman;
///
/// declare_surfman!();
///
/// fn main() { ... }
/// ```
///
/// On Windows, this macro exports various linker flags that the GPU drivers look at to determine
/// whether to use the integrated or discrete GPU. If you don't use this macro, `surfman` should
/// still work, but you may get the wrong GPU.
#[macro_export]
macro_rules! declare_surfman {
    () => {
        #[cfg(target_os = "windows")]
        #[link_section = ".drectve"]
        #[no_mangle]
        pub static _SURFMAN_LINK_ARGS: [u8; 74] =
            *b" /export:NvOptimusEnablement /export:AmdPowerXpressRequestHighPerformance ";
        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub static mut NvOptimusEnablement: i32 = 1;
        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub static mut AmdPowerXpressRequestHighPerformance: i32 = 1;
    };
}
