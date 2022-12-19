// surfman/android-example/rust/src/lib.rs

#[macro_use]
extern crate log;

use crate::threads::common::ResourceLoader;
use crate::threads::App;

use android_logger::Config;
use euclid::default::Size2D;
use jni::objects::{GlobalRef, JByteBuffer, JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};
use log::Level;
use std::cell::{Cell, RefCell};
use std::thread::{self, JoinHandle};
use std::{mem, slice};
use surfman::platform::android::tests;
use surfman::{Connection, NativeContext, NativeDevice};

#[path = "../../../surfman/examples/threads.rs"]
mod threads;

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
    static ATTACHED_TO_JNI: Cell<bool> = Cell::new(false);
}

// Im confused NativeDevice::current() does not exist
/** #[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsExampleRenderer_init(
    env: JNIEnv,
    _class: JClass,
    loader: JObject,
    width: i32,
    height: i32,
) {
    ATTACHED_TO_JNI.with(|attached_to_jni| attached_to_jni.set(true));

    android_logger::init_once(Config::default().with_min_level(Level::Trace));

    let window_size = Size2D::new(width, height);

    let connection = Connection::new().unwrap();
    let device = connection
        .create_device_from_native_device(NativeDevice::current())
        .unwrap();
    let context = device
        .create_context_from_native_context(NativeContext::current().unwrap())
        .unwrap();
    let adapter = device.adapter();

    APP.with(|app| {
        let resource_loader = Box::new(JavaResourceLoader::new(env, loader));
        *app.borrow_mut() = Some(App::new(
            connection,
            adapter,
            device,
            context,
            resource_loader,
            window_size,
        ))
    });
} **/
#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsExampleRenderer_tick(
    _env: JNIEnv,
    _class: JClass,
) {
    APP.with(|app| app.borrow_mut().as_mut().unwrap().tick(false));
}

// NB: New tests should be added here.

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testContextCreation(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_context_creation();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testCrossDeviceSurfaceTextureBlitFramebuffer(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_cross_device_surface_texture_blit_framebuffer();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testCrossThreadSurfaceTextureBlitFramebuffer(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_cross_thread_surface_texture_blit_framebuffer();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testDeviceAccessors(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_device_accessors();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testDeviceCreation(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_device_creation();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testGenericSurfaceCreation(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_generic_surface_creation();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testGL(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_gl();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testNewlyCreatedContextsAreNotCurrent(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_newly_created_contexts_are_not_current();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testSurfaceTextureBlitFramebuffer(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_surface_texture_blit_framebuffer();
}

#[no_mangle]
pub unsafe extern "system" fn Java_org_mozilla_surfmanthreadsexample_SurfmanInstrumentedTest_testSurfaceTextureRightSideUp(
    _env: JNIEnv,
    _class: JClass,
) {
    tests::test_surface_texture_right_side_up();
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
                let slice = unsafe {
                    slice::from_raw_parts(env.get_direct_buffer_address(byte_buffer).unwrap(), 1)
                };
                dest.extend_from_slice(slice);
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
