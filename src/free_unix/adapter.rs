/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A hardware display adapter on Wayland / X11 systems.

use crate::{AdapterPreferences, PowerPreference, RenderingPreference};
use std::env;

static MESA_SOFTWARE_RENDERING_ENV_VAR: &str = "LIBGL_ALWAYS_SOFTWARE";
static MESA_DRI_PRIME_ENV_VAR: &str = "DRI_PRIME";

/// An implementation of [`crate::Adapter`] for Wayland / X11 platforms.
#[derive(Clone, Debug)]
pub struct FreeUnixAdapter(AdapterPreferences);

impl FreeUnixAdapter {
    #[inline]
    pub(crate) fn new(preferences: AdapterPreferences) -> Self {
        Self(preferences)
    }

    pub(crate) fn set_environment_variables(&self) {
        env::remove_var(MESA_SOFTWARE_RENDERING_ENV_VAR);
        env::remove_var(MESA_DRI_PRIME_ENV_VAR);
        match self.0.rendering {
            RenderingPreference::Hardware => match self.0.power {
                PowerPreference::High => env::set_var(MESA_DRI_PRIME_ENV_VAR, "1"),
                PowerPreference::Low => {}
            },
            RenderingPreference::Software => env::set_var(MESA_SOFTWARE_RENDERING_ENV_VAR, "1"),
        }
    }
}
