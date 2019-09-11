//! A stub implementation of a hardware surface that reports errors when methods are invoked on it.

use super::context::ContextDescriptor;

pub struct Surface;

impl Surface {
    #[inline]
    pub fn descriptor(&self) -> ContextDescriptor {
        unimplemented!()
    }
}
