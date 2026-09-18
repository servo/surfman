/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A hardware display adapter on Wayland / X11 systems.

use std::env;

static MESA_SOFTWARE_RENDERING_ENV_VAR: &str = "LIBGL_ALWAYS_SOFTWARE";
static MESA_DRI_PRIME_ENV_VAR: &str = "DRI_PRIME";

/// An implementation of [`crate::Adapter`] for Wayland / X11 platforms.
#[derive(Clone, Debug)]
pub enum FreeUnixAdapter {
    #[doc(hidden)]
    Hardware,
    #[doc(hidden)]
    HardwarePrime,
    #[doc(hidden)]
    Software,
}

impl FreeUnixAdapter {
    #[inline]
    pub(crate) fn hardware() -> Self {
        Self::HardwarePrime
    }

    #[inline]
    pub(crate) fn low_power() -> Self {
        Self::Hardware
    }

    #[inline]
    pub(crate) fn software() -> Self {
        Self::Software
    }

    pub(crate) fn set_environment_variables(&self) {
        match *self {
            Self::Hardware | Self::HardwarePrime => {
                env::remove_var(MESA_SOFTWARE_RENDERING_ENV_VAR);
            }
            Self::Software => {
                env::set_var(MESA_SOFTWARE_RENDERING_ENV_VAR, "1");
            }
        }

        match *self {
            Self::Software => {}
            Self::Hardware => {
                env::remove_var(MESA_DRI_PRIME_ENV_VAR);
            }
            Self::HardwarePrime => {
                env::set_var(MESA_DRI_PRIME_ENV_VAR, "1");
            }
        }
    }
}
