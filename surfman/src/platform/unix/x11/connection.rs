// surfman/surfman/src/platform/unix/x11/connection.rs
//
//! A wrapper for X11 server connections (`DISPLAY` variables).
//!
//! FIXME(pcwalton): I think this should actually wrap the `Display`.

#[derive(Clone)]
pub struct Connection {
    pub(crate) display_name: Option<CString>,
}

impl Connection {
    /// Connects to the default display.
    #[inline]
    pub fn new() -> Result<Connection, Error> {
        Ok(Connection { display_name: None })
    }
}
