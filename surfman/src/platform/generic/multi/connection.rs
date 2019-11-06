// surfman/surfman/src/platform/generic/multi/connection.rs
//
//! A connection abstraction that allows the choice of backends dynamically.

use crate::Error;
use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;
use super::adapter::Adapter;
use super::device::Device;
use super::surface::NativeWidget;

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

impl<Def, Alt> Connection<Def, Alt>
               where Def: DeviceInterface,
                     Alt: DeviceInterface,
                     Def::Connection: ConnectionInterface<Device = Def>,
                     Alt::Connection: ConnectionInterface<Device = Alt> {
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

    pub fn create_device(&self, adapter: &Adapter<Def, Alt>) -> Result<Device<Def, Alt>, Error> {
        match (self, adapter) {
            (&Connection::Default(ref connection), &Adapter::Default(ref adapter)) => {
                connection.create_device(adapter).map(Device::Default)
            }
            (&Connection::Alternate(ref connection), &Adapter::Alternate(ref adapter)) => {
                connection.create_device(adapter).map(Device::Alternate)
            }
            _ => Err(Error::IncompatibleAdapter),
        }
    }

    #[cfg(feature = "sm-winit")]
    pub fn from_winit_window(window: &Window) -> Result<Connection<Def, Alt>, Error> {
        match <Def::Connection>::from_winit_window(window) {
            Ok(connection) => Ok(Connection::Default(connection)),
            Err(_) => <Alt::Connection>::from_winit_window(window).map(Connection::Alternate),
        }
    }

    #[cfg(feature = "sm-winit")]
    pub fn create_native_widget_from_winit_window(&self, window: &Window)
                                                  -> Result<NativeWidget<Def, Alt>, Error> {
        match *self {
            Connection::Default(ref connection) => {
                connection.create_native_widget_from_winit_window(window)
                          .map(NativeWidget::Default)
            }
            Connection::Alternate(ref connection) => {
                connection.create_native_widget_from_winit_window(window)
                          .map(NativeWidget::Alternate)
            }
        }
    }
}

impl<Def, Alt> ConnectionInterface for Connection<Def, Alt>
                                   where Def: DeviceInterface,
                                         Alt: DeviceInterface,
                                         Def::Connection: ConnectionInterface<Device = Def>,
                                         Alt::Connection: ConnectionInterface<Device = Alt> {
    type Adapter = Adapter<Def, Alt>;
    type Device = Device<Def, Alt>;
    type NativeWidget = NativeWidget<Def, Alt>;

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
    fn create_device(&self, adapter: &Adapter<Def, Alt>) -> Result<Device<Def, Alt>, Error> {
        Connection::create_device(self, adapter)
    }

    #[inline]
    #[cfg(feature = "sm-winit")]
    fn from_winit_window(window: &Window) -> Result<Connection<Def, Alt>, Error> {
        Connection::from_winit_window(window)
    }

    #[cfg(feature = "sm-winit")]
    fn create_native_widget_from_winit_window(&self, window: &Window)
                                              -> Result<Self::NativeWidget, Error> {
        Connection::create_native_widget_from_winit_window(self, window)
    }
}
