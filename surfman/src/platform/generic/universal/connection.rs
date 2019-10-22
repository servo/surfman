// surfman/surfman/src/platform/generic/universal/connection.rs
//
//! A window server connection for the universal device.

use crate::Error;
use crate::platform::default::connection::Connection as PlatformConnection;

#[derive(Clone)]
pub enum Connection {
    Some(PlatformConnection),
    None,
}

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        match PlatformConnection::new() {
            Ok(platform_connection) => Ok(Connection::Some(platform_connection)),
            Err(_) => Ok(Connection::None),
        }
    }
}
