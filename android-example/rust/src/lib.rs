// surfman/android-example/rust/src/lib.rs

use crate::threads::App;

use jni::JNIEnv;
use jni::objects::{JClass, JObject};
use std::cell::RefCell;
use surfman::{Adapter, Device};

#[path = "../../../surfman/examples/threads.rs"]
mod threads;

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsRenderer_init(
    env: JNIEnv,
    class: JClass,
    activity: JObject,
    loader: JObject,
    width: i32,
    height: i32,
) {
    let adapter = Adapter::default().unwrap();
    let (device, context) = Device::from_current_context().unwrap();

    APP.with(|app| *app.borrow_mut() = Some(App::new(adapter, device, context)));
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsRenderer_tick(
    env: JNIEnv,
    class: JClass,
) {
    APP.with(|app| app.borrow_mut().as_mut().unwrap().tick());
}
