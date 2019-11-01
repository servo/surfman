// surfman/surfman/src/adapter.rs
//
//! The abstract interface that all adapters conform to.

use crate::Error;

pub trait Adapter: Sized {
    fn default() -> Result<Self, Error>;
    fn hardware() -> Result<Self, Error>;
    fn software() -> Result<Self, Error>;
}
