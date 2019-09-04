//! A thread-local handle to the device.

pub struct Device {
    egl_device: EGLDeviceEXT,
    pub egl_display: EGLDisplay,
    surfaces: Vec<Surface>,
    owned_by_us: bool,
}
