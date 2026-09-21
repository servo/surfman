/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A platform-independent representation of a display adapter.

#[cfg(all(windows_platform, feature = "sm-angle"))]
use crate::angle::adapter::AngleAdapter;
#[cfg(macos_platform)]
use crate::base::io_surface::adapter::AppleAdapter;
#[cfg(free_unix)]
use crate::free_unix::adapter::FreeUnixAdapter;
#[cfg(any(android_platform, ohos_platform))]
use crate::hardware_buffer::adapter::HardwareBufferAdapter;
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
use crate::wgl::adapter::WglAdapter;
use crate::{macros::enum_conversion, Error};

/// A power usage preference for selecting an adapter.
#[derive(Copy, Clone, Debug, Default)]
pub enum PowerPreference {
    /// Prefer a high-performance adapter.
    #[default]
    HighPerformance,
    /// Prefer a low-power adapter.
    LowPower,
}

/// A hardware/software preference for selecting an adapter.
#[derive(Copy, Clone, Debug, Default)]
pub enum RenderingPreference {
    /// Prefer a hardware adapter.
    #[default]
    Hardware,
    /// Prefer a software adapter.
    Software,
}

/// A set of options to use when choosing an adapter. The default preference is
/// for a high-power hardware adapter.
#[derive(Copy, Clone, Debug, Default)]
pub struct AdapterPreferences {
    /// The [`PowerPreference`] for choosing an adapter.
    pub power: PowerPreference,
    /// The [`RenderingPreference`] for choosing an adapter.
    pub rendering: RenderingPreference,
}

/// Represents a hardware display adapter that can be used for rendering (including the CPU).
///
/// Adapters can be sent between threads. To render with an adapter, open a thread-local `Device`.
#[derive(Debug, Clone)]
pub enum Adapter {
    /// An ANGLE adapter for Windows systems.
    #[cfg(all(windows_platform, feature = "sm-angle"))]
    Angle(AngleAdapter),
    /// An adapter for Apple systems.
    #[cfg(macos_platform)]
    Apple(AppleAdapter),
    /// A generic EGL adapter for Wayland / X11 systems.
    #[cfg(free_unix)]
    FreeUnix(FreeUnixAdapter),
    /// A hardware buffer adapter for OHOS and Android systems.
    #[cfg(any(android_platform, ohos_platform))]
    HardwareBuffer(HardwareBufferAdapter),
    /// A WGL adapter for Windows systems.
    #[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
    Wgl(WglAdapter),
}

#[cfg(all(windows_platform, feature = "sm-angle"))]
enum_conversion!(Adapter, Angle, AngleAdapter, angle);
#[cfg(macos_platform)]
enum_conversion!(Adapter, Apple, AppleAdapter, apple);
#[cfg(free_unix)]
enum_conversion!(Adapter, FreeUnix, FreeUnixAdapter, free_unix);
#[cfg(any(android_platform, ohos_platform))]
enum_conversion!(
    Adapter,
    HardwareBuffer,
    HardwareBufferAdapter,
    hardware_buffer
);
#[cfg(all(windows_platform, not(feature = "sm-no-wgl")))]
enum_conversion!(Adapter, Wgl, WglAdapter, wgl);
