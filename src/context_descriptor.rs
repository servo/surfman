/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[cfg(macos_platform)]
use crate::cgl::context::CglContextDescriptor;
use crate::macros::enum_conversion;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::context::WglContextDescriptor;
#[cfg(any(android_platform, angle, free_unix, ohos_platform))]
use crate::EglContextDescriptor;

/// Information needed to create a context. Some APIs call this a "config" or a "pixel format".
///
/// These are local to a device.
#[derive(Clone)]
pub enum ContextDescriptor {
    /// A [`ContextDescriptor`] for CGL platforms.
    #[cfg(macos_platform)]
    Cgl(CglContextDescriptor),
    /// A [`ContextDescriptor`] for EGL platforms.
    #[cfg(any(android_platform, angle, free_unix, ohos_platform))]
    Egl(EglContextDescriptor),
    /// A [`ContextDescriptor`] for WGL platforms.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglContextDescriptor),
}

#[cfg(macos_platform)]
enum_conversion!(
    ContextDescriptor,
    Cgl,
    CglContextDescriptor,
    cgl,
    IncompatibleContextDescriptor
);
#[cfg(any(android_platform, angle, free_unix, ohos_platform))]
enum_conversion!(
    ContextDescriptor,
    Egl,
    EglContextDescriptor,
    egl,
    IncompatibleContextDescriptor
);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(
    ContextDescriptor,
    Wgl,
    WglContextDescriptor,
    wgl,
    IncompatibleContextDescriptor
);
