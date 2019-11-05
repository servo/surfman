// surfman/surfman/src/platform/generic/multi/connection.rs
//
//! A connection abstraction that allows the choice of backends dynamically.

use crate::Error;
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use super::adapter::Adapter;

#[cfg(feature = "sm-winit")]
use winit::Window;

#[derive(Clone)]
pub enum Connection<Def, Alt> where Def: DeviceInterface,
                                    Alt: DeviceInterface,
                                    Def::Connection: ConnectionInterface,
                                    Alt::Connection: ConnectionInterface {
    Default(Def::Connection),
    Alternate(Alt::Connection),
}

impl<Def, Alt> Connection<Def, Alt> where Def: DeviceInterface,
                                          Alt: DeviceInterface,
                                          Def::Connection: ConnectionInterface,
                                          Alt::Connection: ConnectionInterface {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection<Def, Alt>, Error> {
        match <Def::Connection>::new() {
            Ok(connection) => Ok(Connection::Default(connection)),
            Err(_) => <Alt::Connection>::new().map(Connection::Alternate),
        }
    }

    pub fn create_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_adapter().map(Adapter::Alternate)
            }
        }
    }

    pub fn create_hardware_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_hardware_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_hardware_adapter().map(Adapter::Alternate)
            }
        }
    }

    pub fn create_software_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_software_adapter().map(Adapter::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_software_adapter().map(Adapter::Alternate)
            }
        }
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection<Def, Alt>, Error> {
        match <Def::Connection>::from_winit_window(window) {
            Ok(connection) => Ok(Connection::Default(connection)),
            Err(_) => <Alt::Connection>::from_winit_window(window).map(Connection::Alternate),
        }
    }
}

impl<Def, Alt> ConnectionInterface for Connection<Def, Alt>
                                   where Def: DeviceInterface,
                                         Alt: DeviceInterface,
                                         Def::Connection: ConnectionInterface,
                                         Alt::Connection: ConnectionInterface {
    type Adapter = Adapter<Def, Alt>;

    #[inline]
    fn new() -> Result<Connection<Def, Alt>, Error> {
        Connection::new()
    }

    #[inline]
    fn create_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_adapter(self)
    }

    #[inline]
    fn create_hardware_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_hardware_adapter(self)
    }

    #[inline]
    fn create_software_adapter(&self) -> Result<Adapter<Def, Alt>, Error> {
        Connection::create_software_adapter(self)
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Connection<Def, Alt>, Error> {
        Connection::from_winit_window(window)
    }
}
