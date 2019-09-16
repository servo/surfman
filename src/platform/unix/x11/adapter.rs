//! A wrapper for X11 adapters (`DISPLAY` variables).

use crate::Error;
use std::ffi::CString;

#[derive(Clone, Debug)]
pub struct Adapter {
    pub(crate) display_name: Option<CString>,
}

impl Adapter {
    /// Returns the "best" adapter on this system.
    #[inline]
    pub fn default() -> Result<Adapter, Error> {
        Ok(Adapter { None })
    }
}
