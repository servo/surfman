// surfman/android-example/rust/src/lib.rs

#[macro_use]
extern crate log;

use crate::threads::App;
use crate::threads::common::ResourceLoader;

use android_logger::Config;
use jni::objects::{GlobalRef, JByteBuffer, JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use log::Level;
use std::cell::{Cell, RefCell};
use std::mem;
use surfman::{Adapter, Device};

#[path = "../../../surfman/examples/threads.rs"]
mod threads;

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
    static ATTACHED_TO_JNI: Cell<bool> = Cell::new(false);
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsExampleRenderer_init(
    env: JNIEnv,
    class: JClass,
    loader: JObject,
    width: i32,
    height: i32,
) {
    android_logger::init_once(Config::default().with_tag("SurfmanThreadsExample")
                                               .with_min_level(Level::Trace));

    error!("init()");

    ATTACHED_TO_JNI.with(|attached_to_jni| attached_to_jni.set(true));

    let adapter = Adapter::default().unwrap();
    let (device, context) = Device::from_current_context().unwrap();

    APP.with(|app| {
        let resource_loader = Box::new(JavaResourceLoader::new(env, loader));
        *app.borrow_mut() = Some(App::new(adapter, device, context, resource_loader))
    });
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsExampleRenderer_tick(
    env: JNIEnv,
    class: JClass,
) {
    error!("tick()");
    APP.with(|app| app.borrow_mut().as_mut().unwrap().tick(false));
}

struct JavaResourceLoader {
    loader: GlobalRef,
    vm: JavaVM,
}

impl ResourceLoader for JavaResourceLoader {
    fn slurp(&self, dest: &mut Vec<u8>, filename: &str) {
        ATTACHED_TO_JNI.with(|attached_to_jni| {
            if !attached_to_jni.get() {
                mem::forget(self.vm.attach_current_thread().unwrap());
                attached_to_jni.set(true);
            }
        });

        let loader = self.loader.as_obj();
        let env = self.vm.get_env().unwrap();
        match env
            .call_method(
                loader,
                "slurp",
                "(Ljava/lang/String;)Ljava/nio/ByteBuffer;",
                &[JValue::Object(*env.new_string(filename).unwrap())],
            )
            .unwrap()
        {
            JValue::Object(object) => {
                let byte_buffer = JByteBuffer::from(object);
                dest.extend_from_slice(env.get_direct_buffer_address(byte_buffer).unwrap());
            }
            _ => panic!("Unexpected return value!"),
        }
    }
}

impl JavaResourceLoader {
    fn new(env: JNIEnv, loader: JObject) -> JavaResourceLoader {
        JavaResourceLoader {
            loader: env.new_global_ref(loader).unwrap(),
            vm: env.get_java_vm().unwrap(),
        }
    }
}