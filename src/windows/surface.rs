//! Surfaces for the Windows backends.

#[cfg(angle)]
use crate::angle::surface::{AngleBufferBinding, AngleWindowBinding};
use crate::{ContextID, SurfaceID, SurfaceInfo};

use euclid::default::Size2D;
use std::fmt::{self, Debug, Formatter};
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::HWND;

/// An offscreen surface backed by a Direct3D 11 texture.
///
/// The texture can be opened by other Direct3D 11 devices through its DXGI share handle.
pub struct Direct3dSurface {
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) share_handle: HANDLE,
    pub(crate) binding: Direct3dBinding,
}

/// The objects that bind a [`Direct3dSurface`] to the context of the backend that created it.
pub(crate) enum Direct3dBinding {
    #[cfg(angle)]
    Angle(AngleBufferBinding),
}

/// A surface displayed in a Win32 window.
pub struct Win32Surface {
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) window: HWND,
    pub(crate) binding: Win32Binding,
}

/// The objects that bind a [`Win32Surface`] to the context of the backend that created it.
pub(crate) enum Win32Binding {
    #[cfg(angle)]
    Angle(AngleWindowBinding),
}

unsafe impl Send for Direct3dSurface {}
unsafe impl Send for Win32Surface {}

impl Direct3dSurface {
    /// Returns the size of the surface.
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    /// Returns the DXGI share handle of the Direct3D 11 texture.
    #[inline]
    pub fn share_handle(&self) -> HANDLE {
        self.share_handle
    }

    pub(crate) fn id(&self) -> SurfaceID {
        match self.binding {
            #[cfg(angle)]
            Direct3dBinding::Angle(ref binding) => SurfaceID(binding.egl_surface as usize),
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: match self.binding {
                #[cfg(angle)]
                Direct3dBinding::Angle(_) => None,
            },
        }
    }
}

impl Win32Surface {
    /// Returns the size of the surface.
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    /// Returns the window this surface is displayed in.
    #[inline]
    pub fn window(&self) -> HWND {
        self.window
    }

    pub(crate) fn id(&self) -> SurfaceID {
        match self.binding {
            #[cfg(angle)]
            Win32Binding::Angle(ref binding) => SurfaceID(binding.egl_surface as usize),
        }
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size,
            id: self.id(),
            context_id: self.context_id,
            framebuffer_object: None,
        }
    }
}

impl Debug for Direct3dSurface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}

impl Debug for Win32Surface {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Surface({:x})", self.id().0)
    }
}
