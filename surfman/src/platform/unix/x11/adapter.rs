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
        Ok(Adapter { display_name: None })
    }

    /// Returns the "best" hardware adapter on this system.
    #[inline]
    pub fn hardware() -> Result<Adapter, Error> {
        Adapter::default()
    }

    /// Returns the "best" software adapter on this system.
    ///
    /// The X11 backend has no software support, so this returns an error. You can use the
    /// universal backend to get a software adapter.
    ///
    /// TODO(pcwalton): If Mesa is in use, maybe we could use `llvmpipe` somehow?
    #[inline]
    pub fn software() -> Result<Adapter, Error> {
        Err(Error::NoSoftwareAdapters)
    }
}
