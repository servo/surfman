// surfman/android-example/rust/src/lib.rs

use crate::threads::App;
use crate::threads::common::ResourceLoader;

use egl::{self, EGL_DRAW, EGL_NO_SURFACE, EGL_READ};
use jni::objects::{GlobalRef, JByteBuffer, JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};
use std::cell::{Cell, RefCell};
use std::mem;
use surfman::{Connection, NativeContext, NativeDevice};

#[path = "../../../surfman/examples/threads.rs"]
mod threads;

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
    static ATTACHED_TO_JNI: Cell<bool> = Cell::new(false);
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsExampleRenderer_init(env: JNIEnv,
                                                                                  _class: JClass,
                                                                                  loader: JObject,
                                                                                  width: i32,
                                                                                  height: i32) {
    ATTACHED_TO_JNI.with(|attached_to_jni| attached_to_jni.set(true));

    let connection = Connection::new().unwrap();
    let native_device = NativeDevice(egl::get_current_display().unwrap());
    let device = connection.create_device_from_native_device(native_device).unwrap();
    let native_context = NativeContext {
        egl_context: egl::get_current_context().unwrap(),
        egl_read_surface: egl::get_current_surface(EGL_READ).unwrap_or(EGL_NO_SURFACE),
        egl_draw_surface: egl::get_current_surface(EGL_DRAW).unwrap_or(EGL_NO_SURFACE),
    };
    let context = device.create_context_from_native_context(native_context).unwrap();
    let adapter = device.adapter();

    APP.with(|app| {
        let resource_loader = Box::new(JavaResourceLoader::new(env, loader));
        *app.borrow_mut() = Some(App::new(connection, adapter, device, context, resource_loader))
    });
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_org_mozilla_surfmanthreadsexample_SurfmanThreadsExampleRenderer_tick(_env: JNIEnv,
                                                                                  _class: JClass) {
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