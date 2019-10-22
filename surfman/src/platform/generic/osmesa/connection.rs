// surfman/surfman/src/platform/src/osmesa/connection.rs
//
//! A no-op connection. OSMesa needs no connection, as it is a CPU-based off-screen-only API.

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
