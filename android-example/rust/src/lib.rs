// surfman/android-example/rust/src/lib.rs

use surfman::{Adapter, Device};

#[path = "../../../surfman/examples/threads.rs"]
mod threads;

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsRenderer_init(
    env: JNIEnv,
    class: JClass,
    activity: JObject,
    loader: JObject,
    width: i32,
    height: i32,
) {
    let (device, context) = Device::from_current_context().unwrap();
    let adapter = Adapter::default();

    APP.with(|app| *app.borrow_mut() = Some(App::new(adapter, device, context)));
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsRenderer_tick(
    env: JNIEnv,
    class: JClass,
) {
    APP.with(|app| app.borrow_mut().tick());
}
