/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! An off-screen surface on Wayland / X11 systems.

use crate::base::egl::surface::EGLBufferSurface;

/// An implementation of [`crate::OffscreenSurface`] for Wayland / X11 platforms, backed by an EGL
/// image of an OpenGL texture.
#[derive(Debug)]
pub struct PbufferSurface(pub(crate) EGLBufferSurface);
