// surfman/surfman/src/platform/generic/multi/adapter.rs
//
//! An adapter abstraction that allows the choice of backends dynamically.

use crate::connection::Connection as ConnectionInterface;
use crate::device::Device as DeviceInterface;

pub enum Adapter<Def, Alt> where Def: DeviceInterface, Alt: DeviceInterface {
    Default(<Def::Connection as ConnectionInterface>::Adapter),
    Alternate(<Alt::Connection as ConnectionInterface>::Adapter),
}
