//! A stub implementation of a hardware surface that reports errors when methods are invoked on it.

use crate::SurfaceDescriptor;

pub struct Surface;

impl Surface {
    #[inline]
    pub fn descriptor(&self) -> &SurfaceDescriptor {
        unimplemented!()
    }
}
