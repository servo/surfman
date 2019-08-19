//! COM utilities.

pub(crate) struct ComPtr<T> {
    ptr: *mut T;
}

impl<T> ComPtr<T> {
    /// Creates a new `ComPtr` without adjusting the underlying reference count.
    pub(crate) unsafe fn new_create(ptr: *mut T) -> ComPtr<T> {

    }
}
