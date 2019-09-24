//! Information related to hardware surfaces.

use std::fmt::{self, Display, Formatter};

// The default framebuffer for a context.
pub(crate) enum Framebuffer<S> {
    // No framebuffer has been attached to the context.
    None,
    // The context is externally-managed.
    External,
    // The context renders to a surface.
    Surface(S),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceID(pub usize);

impl Display for SurfaceID {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}
