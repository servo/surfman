//! Surfaces shared by the macOS backends.

use crate::cgl::surface::CglBinding;
use crate::{ContextID, Error, SurfaceID, SurfaceInfo};

use euclid::default::Size2D;
use objc2_core_foundation::CFRetained;
use objc2_io_surface::IOSurfaceRef;
use std::fmt::{self, Debug, Formatter};

/// A surface backed by an `IOSurface`, which can be bound to a texture.
///
/// The `IOSurface` can be used by other devices, graphics APIs and processes.
pub struct IoSurfaceSurface {
    pub(crate) io_surface: CFRetained<IOSurfaceRef>,
    pub(crate) binding: IoSurfaceBinding,
}

/// The objects that bind an [`IoSurfaceSurface`] to the context of the backend that created it.
pub(crate) enum IoSurfaceBinding {
    Cgl(CglBinding),
}

unsafe impl Send for IoSurfaceSurface {}

impl IoSurfaceSurface {
    /// Returns the size of the surface.
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        Size2D::new(
            self.io_surface.width() as i32,
            self.io_surface.height() as i32,
        )
    }

    /// Returns the `IOSurface` that holds the surface's pixels.
    #[inline]
    pub fn io_surface(&self) -> CFRetained<IOSurfaceRef> {
        self.io_surface.clone()
    }

    pub(crate) fn cgl(&self) -> Result<&CglBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            IoSurfaceBinding::Cgl(ref binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn cgl_mut(&mut self) -> Result<&mut CglBinding, Error> {
        #[allow(unreachable_patterns)]
        match self.binding {
            IoSurfaceBinding::Cgl(ref mut binding) => Ok(binding),
            _ => Err(Error::IncompatibleSurface),
        }
    }

    pub(crate) fn context_id(&self) -> ContextID {
        match self.binding {
            IoSurfaceBinding::Cgl(ref binding) => binding.context_id,
        }
    }

    pub(crate) fn id(&self) -> SurfaceID {
        SurfaceID(&*self.io_surface as *const IOSurfaceRef as usize)
    }

    pub(crate) fn info(&self) -> SurfaceInfo {
        SurfaceInfo {
            size: self.size(),
            id: self.id(),
            context_id: self.context_id(),
            framebuffer_object: match self.binding {
                IoSurfaceBinding::Cgl(ref binding) => binding.framebuffer_object,
            },
        }
    }
}

impl Debug for IoSurfaceSurface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "IoSurfaceSurface({:x})", self.id().0)
    }
}
