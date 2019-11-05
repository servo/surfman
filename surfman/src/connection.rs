// surfman/surfman/src/connection.rs
//
//! The abstract interface that all connections conform to.

use crate::Error;

#[cfg(feature = "sm-winit")]
use winit::Window;

pub trait Connection: Sized {
    type Adapter;

    fn new() -> Result<Self, Error>;

    fn create_adapter(&self) -> Result<Self::Adapter, Error>;
    fn create_hardware_adapter(&self) -> Result<Self::Adapter, Error>;
    fn create_software_adapter(&self) -> Result<Self::Adapter, Error>;

    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Self, Error>;
}
