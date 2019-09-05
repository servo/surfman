/// Common functionality across graphics contexts.

pub trait ReleaseContext {
    type Context;
    unsafe fn release(&mut self, context: Self::Context);
}
