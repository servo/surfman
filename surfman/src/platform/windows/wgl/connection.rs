// surfman/surfman/src/platform/windows/wgl/connection.rs
//
//! A connection to the window server.
//! 
//! Window server handles are implicit in the Win32 API, so this is a no-op.

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
