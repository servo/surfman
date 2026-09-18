/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A hardware display adapter for Apple systems.

/// An implementation of [`crate::Adapter`] for Apple platforms.
#[derive(Clone, Debug)]
pub struct AppleAdapter {
    pub(crate) is_low_power: bool,
}
