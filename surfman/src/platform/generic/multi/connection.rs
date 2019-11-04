// surfman/surfman/src/platform/generic/multi/connection.rs
//
//! A connection abstraction that allows the choice of backends dynamically.

use crate::Error;
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;

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
    #[inline]
    fn new() -> Result<Connection<Def, Alt>, Error> {
        Connection::new()
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Connection<Def, Alt>, Error> {
        Connection::from_winit_window(window)
    }
}
