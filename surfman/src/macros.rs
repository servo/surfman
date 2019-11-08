// surfman/src/macros.rs
//
//! Macros.

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
    }
}

