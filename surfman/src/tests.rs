// surfman/surfman/src/tests.rs
//
//! Unit tests.

use crate::{ContextAttributeFlags, ContextAttributes, Error, GLApi, GLVersion, WindowingApiError};
use super::connection::Connection;
use super::device::Adapter;

static GL_VERSIONS: [GLVersion; 6] = [
    GLVersion { major: 2, minor: 0 },
    GLVersion { major: 3, minor: 0 },
    GLVersion { major: 3, minor: 1 },
    GLVersion { major: 3, minor: 2 },
    GLVersion { major: 4, minor: 0 },
    GLVersion { major: 4, minor: 1 },
];

static GL_ES_VERSIONS: [GLVersion; 4] = [
    GLVersion { major: 2, minor: 0 },
    GLVersion { major: 3, minor: 0 },
    GLVersion { major: 3, minor: 1 },
    GLVersion { major: 3, minor: 2 },
];

#[test]
fn test_adapter_creation() {
    let connection = Connection::new().unwrap();
    connection.create_hardware_adapter().unwrap();
    connection.create_low_power_adapter().unwrap();
    connection.create_software_adapter().unwrap();
}

#[test]
fn test_device_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };
}

#[test]
fn test_device_accessors() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().unwrap();
    let device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };
    drop(device.connection());
    drop(device.adapter());
    drop(device.gl_api());
}

// Tests that all combinations of flags result in the creation of valid context descriptors and
// contexts.
#[test]
fn test_context_creation() {
    let connection = Connection::new().unwrap();
    let adapter = connection.create_low_power_adapter().expect("Failed to create adapter!");
    let mut device = match connection.create_device(&adapter) {
        Ok(device) => device,
        Err(Error::RequiredExtensionUnavailable) => {
            // Can't run these tests on this hardware.
            return;
        }
        Err(err) => panic!("Failed to create device: {:?}", err),
    };

    let versions = match device.gl_api() {
        GLApi::GL => &GL_VERSIONS[..],
        GLApi::GLES => &GL_ES_VERSIONS[..],
    };

    for &version in versions {
        for flag_bits in 0..(ContextAttributeFlags::all().bits() + 1) {
            let flags = ContextAttributeFlags::from_bits_truncate(flag_bits);
            let attributes = ContextAttributes { version, flags };
            println!("Creating context with attributes: {:?}", attributes);
            let descriptor = match device.create_context_descriptor(&attributes) {
                Ok(descriptor) => descriptor,
                Err(Error::UnsupportedGLProfile) | Err(Error::UnsupportedGLVersion) => {
                    // Nothing we can do about this. Go on to the next one.
                    continue
                }
                Err(err) => panic!("Context descriptor creation failed: {:?}", err),
            };

            match device.create_context(&descriptor) {
                Ok(mut context) => {
                    // Verify that the attributes round-trip.
                    let actual_descriptor = device.context_descriptor(&context);
                    let actual_attributes =
                        device.context_descriptor_attributes(&actual_descriptor);
                    if !actual_attributes.flags.contains(attributes.flags) {
                        device.destroy_context(&mut context).unwrap();
                        panic!("Expected at least attribute flags {:?} but got {:?}",
                               attributes.flags,
                               actual_attributes.flags);
                    }
                    if actual_attributes.version.major < attributes.version.major ||
                            (actual_attributes.version.major == attributes.version.major &&
                             actual_attributes.version.minor < attributes.version.minor) {
                        device.destroy_context(&mut context).unwrap();
                        panic!("Expected at least GL version {:?} but got version {:?}",
                               attributes,
                               actual_attributes);
                    }

                    device.destroy_context(&mut context).unwrap();
                }
                Err(Error::ContextCreationFailed(WindowingApiError::BadPixelFormat)) => {
                    // This is OK, as it just means the GL implementation didn't support the
                    // requested GL version.
                }
                Err(error) => panic!("Failed to create context: {:?}", error),
            }
        }
    }
}
