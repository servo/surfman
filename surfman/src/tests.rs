// surfman/surfman/src/tests.rs
//
//! Unit tests.

use crate::platform::default::adapter::Adapter;
use crate::platform::default::connection::Connection;
use crate::{ContextAttributeFlags, ContextAttributes, Error, GLVersion};

static GL_VERSIONS: [GLVersion; 5] = [
    GLVersion { major: 2, minor: 0 },
    GLVersion { major: 2, minor: 1 },
    GLVersion { major: 3, minor: 0 },
    GLVersion { major: 3, minor: 3 },
    GLVersion { major: 4, minor: 1 },
];

#[test]
fn test_adapter_creation() {
    let connection = Connection::new().unwrap();
    check_hw(connection.create_hardware_adapter());
    check_hw(connection.create_low_power_adapter());
    match connection.create_software_adapter() {
        Ok(_) | Err(Error::NoSoftwareAdapters) => {}
        _ => panic!(),
    }

    fn check_hw(result: Result<Adapter, Error>) {
        match result {
            Ok(_) | Err(Error::NoHardwareAdapters) => {}
            _ => panic!(),
        }
    }
}

#[test]
fn test_device_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().unwrap();
    connection.create_device(&adapter).unwrap();
}

#[test]
fn test_device_accessors() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().unwrap();
    let device = connection.create_device(&adapter).unwrap();
    drop(device.connection());
    drop(device.adapter());
    drop(device.gl_api());
}

// Tests that all combinations of flags result in the creation of valid context descriptors and
// contexts.
#[test]
fn test_context_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().unwrap();
    let mut device = connection.create_device(&adapter).unwrap();

    for &version in &GL_VERSIONS {
        for flag_bits in 0..(ContextAttributeFlags::all().bits() + 1) {
            let flags = ContextAttributeFlags::from_bits_truncate(flag_bits);
            let attributes = ContextAttributes { version, flags };
            let descriptor = device.create_context_descriptor(&attributes).unwrap();
            let mut context = device.create_context(&descriptor).unwrap();
            device.destroy_context(&mut context).unwrap();
        }
    }
}
