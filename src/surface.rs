//! Information related to hardware surfaces.

use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceId(pub usize);

impl Display for SurfaceId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}
