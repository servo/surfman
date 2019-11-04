// surfman/surfman/src/platform/generic/multi/adapter.rs
//
//! An adapter abstraction that allows the choice of backends dynamically.

use crate::Error;
use crate::adapter::Adapter as AdapterInterface;
use crate::device::Device as DeviceInterface;

#[derive(Clone, Debug)]
pub enum Adapter<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(Def::Adapter),
    Alternate(Alt::Adapter),
}

impl<Def, Alt> Adapter<Def, Alt> where Def: DeviceInterface,
                                       Alt: DeviceInterface,
                                       Def::Adapter: AdapterInterface,
                                       Alt::Adapter: AdapterInterface {
    /// Returns the "best" adapter on this system.
    pub fn default() -> Result<Adapter<Def, Alt>, Error> {
        match <Def::Adapter>::default() {
            Ok(adapter) => Ok(Adapter::Default(adapter)),
            Err(_) => <Alt::Adapter>::default().map(Adapter::Alternate),
        }
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn hardware() -> Result<Adapter<Def, Alt>, Error> {
        match <Def::Adapter>::hardware() {
            Ok(adapter) => Ok(Adapter::Default(adapter)),
            Err(_) => <Alt::Adapter>::hardware().map(Adapter::Alternate),
        }
    }

    /// Returns the "best" software adapter on this system.
    #[inline]
    pub fn software() -> Result<Adapter<Def, Alt>, Error> {
        match <Def::Adapter>::software() {
            Ok(adapter) => Ok(Adapter::Default(adapter)),
            Err(_) => <Alt::Adapter>::software().map(Adapter::Alternate),
        }
    }
}

impl<Def, Alt> AdapterInterface for Adapter<Def, Alt> where Def: DeviceInterface,
                                                            Alt: DeviceInterface,
                                                            Def::Adapter: AdapterInterface,
                                                            Alt::Adapter: AdapterInterface {
    #[inline]
    fn default() -> Result<Adapter<Def, Alt>, Error> {
        Adapter::default()
    }

    #[inline]
    fn hardware() -> Result<Adapter<Def, Alt>, Error> {
        Adapter::hardware()
    }

    #[inline]
    fn software() -> Result<Adapter<Def, Alt>, Error> {
        Adapter::software()
    }
}
