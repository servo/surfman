// surfman/surfman/src/connection.rs
//
//! The abstract interface that all connections conform to.

use crate::Error;

#[cfg(feature = "sm-winit")]
use winit::Window;

pub trait Connection: Sized {
    fn new() -> Result<Self, Error>;

    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Self, Error>;
}
