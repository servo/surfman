//! An adapter abstraction that can choose between hardware and software rendering.

use crate::Error;
use crate::platform::default::adapter::Adapter as HWAdapter;
use crate::platform::generic::osmesa::adapter::Adapter as OSMesaAdapter;

#[derive(Clone)]
pub enum Adapter {
    Hardware(HWAdapter),
    Software(OSMesaAdapter),
}

impl Adapter {
    /// Returns the "best" adapter on this system.
    pub fn default() -> Result<Adapter, Error> {
        match Adapter::hardware() {
            Ok(adapter) => Ok(adapter),
            Err(_) => Adapter::software(),
        }
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn hardware() -> Result<Adapter, Error> {
        HWAdapter::default().map(Adapter::Hardware)
    }

    /// Returns the "best" software adapter on this system.
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        OSMesaAdapter::default().map(Adapter::Software)
    }
}
