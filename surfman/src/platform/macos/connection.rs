// surfman/surfman/src/platform/src/macos/connection.rs
//
//! Represents the connection to the Core Graphics window server.
//! 
//! This is a no-op, because the system APIs automatically manage the global window server
//! connection.

use crate::Error;

/// A no-op connection.
#[derive(Clone)]
pub struct Connection;

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection)
    }
}
