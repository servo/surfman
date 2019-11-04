// surfman/surfman/src/implementation/adapter.rs
//
//! This is an included private module that automatically produces the implementation of the
//! `Adapter` trait for a backend.

use crate::Error;
use crate::adapter::Adapter as AdapterInterface;
use super::super::adapter::Adapter;

impl AdapterInterface for Adapter {
    #[inline]
    fn default() -> Result<Adapter, Error> {
        Adapter::default()
    }

    #[inline]
    fn hardware() -> Result<Adapter, Error> {
        Adapter::hardware()
    }

    #[inline]
    fn software() -> Result<Adapter, Error> {
        Adapter::software()
    }
}
