// surfman/surfman/platform/unix/generic/device.rs
//
//! DRI adapters on Unix.
//!
//! These are shared between Wayland and X11 backends.

use std::env;

static MESA_SOFTWARE_RENDERING_ENV_VAR: &'static str = "LIBGL_ALWAYS_SOFTWARE";
static MESA_DRI_PRIME_ENV_VAR: &'static str = "DRI_PRIME";

#[derive(Clone, Debug)]
pub enum Adapter {
    Hardware,
    HardwarePrime,
    Software,
}

impl Adapter {
    #[inline]
    pub(crate) fn hardware() -> Adapter {
        Adapter::HardwarePrime
    }

    #[inline]
    pub(crate) fn low_power() -> Adapter {
        Adapter::Hardware
    }

    #[inline]
    pub(crate) fn software() -> Adapter {
        Adapter::Software
    }

    pub(crate) fn set_environment_variables(&self) {
        match *self {
            Adapter::Hardware | Adapter::HardwarePrime => {
                env::remove_var(MESA_SOFTWARE_RENDERING_ENV_VAR);
            }
            Adapter::Software => {
                env::set_var(MESA_SOFTWARE_RENDERING_ENV_VAR, "1");
            }
        }

        match *self {
            Adapter::Software => {}
            Adapter::Hardware => {
                env::remove_var(MESA_DRI_PRIME_ENV_VAR);
            }
            Adapter::HardwarePrime => {
                env::set_var(MESA_DRI_PRIME_ENV_VAR, "1");
            }
        }
    }
}

