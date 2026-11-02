//! Phone-only glue: the on-screen keyboard, GPS and system bars (through the small Java helpers in android/java).

use jni::JavaVM;
use jni::objects::{GlobalRef, JClass, JDoubleArray, JIntArray, JObject, JValue};
use std::sync::{Mutex, OnceLock};
use winit::platform::android::activity::AndroidApp;

static APP: OnceLock<AndroidApp> = OnceLock::new();
/// Java helper classes, looked up once each.
static CLASSES: Mutex<Vec<(&str, GlobalRef)>> = Mutex::new(Vec::new());

pub fn init(app: AndroidApp) {
    let _ = APP.set(app);
    if let Some(dir) = with_gps(|env, cls, act| {
        let s = env
            .call_static_method(cls, "exportDir", "(Landroid/app/Activity;)Ljava/lang/String;", &[JValue::Object(act)])?
            .l()?;
        let s: String = env.get_string(&s.into())?.into();
        Ok(s)
    }) {
        if !dir.is_empty() {
            // SAFETY: called once at start-up before other threads read the environment.
            unsafe { std::env::set_var("GHOSTWALK_EXPORT", dir) };
        }
    }
}

/// Shows or hides the on-screen keyboard when a text box gains or loses focus.
pub fn keyboard(show: bool) {
    if let Some(app) = APP.get() {
        if show {
            app.show_soft_input(true);
        } else {
            app.hide_soft_input(true);
        }
    }
}

fn with_gps<T>(f: impl FnOnce(&mut jni::JNIEnv, &JClass, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    with_class("id.ghostwalk.app.Gps", f)
}

/// Runs `f` with one of the helper classes in android/java and the running activity.
fn with_class<T>(
    name: &'static str,
    f: impl FnOnce(&mut jni::JNIEnv, &JClass, &JObject) -> jni::errors::Result<T>,
) -> Option<T> {
    let app = APP.get()?;
    // SAFETY: android-activity keeps the VM and a global reference to the running Activity alive
    // while `AndroidApp` exists. (ndk_context's context is the Application, not the Activity, so
    // Activity-only calls such as getWindow() or requestPermissions() must not use it.)
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread().ok()?;
    // A local frame frees every Java reference made here; this runs many times a second.
    env.with_local_frame(16, |env| -> jni::errors::Result<T> {
        let out = (|| {
            let mut classes = CLASSES.lock().unwrap_or_else(|e| e.into_inner());
            if !classes.iter().any(|c| c.0 == name) {
                let loader = env
                    .call_method(&activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
                    .l()?;
                let jname = env.new_string(name)?;
                let cls = env
                    .call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&jname)])?
                    .l()?;
                classes.push((name, env.new_global_ref(cls)?));
            }
            let cls = classes.iter().find(|c| c.0 == name).expect("added above").1.clone();
            drop(classes);
            f(env, cls.as_obj().into(), &activity)
        })();
        if out.is_err() {
            // Print the Java error to the phone log (tag System.err) instead of hiding it.
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        out
    })
    .ok()
}

pub fn gps_start() {
    with_gps(|env, cls, act| {
        env.call_static_method(cls, "start", "(Landroid/app/Activity;)V", &[JValue::Object(act)])?;
        Ok(())
    });
}

pub fn gps_stop() {
    with_gps(|env, cls, act| {
        env.call_static_method(cls, "stop", "(Landroid/app/Activity;)V", &[JValue::Object(act)])?;
        Ok(())
    });
}

/// Opens this app's page in the phone settings (to allow location after saying no).
pub fn gps_open_app_settings() {
    with_gps(|env, cls, act| {
        env.call_static_method(cls, "openAppSettings", "(Landroid/app/Activity;)V", &[JValue::Object(act)])?;
        Ok(())
    });
}

/// Opens the phone's Location switch.
pub fn gps_open_location_settings() {
    with_gps(|env, cls, act| {
        env.call_static_method(cls, "openLocationSettings", "(Landroid/app/Activity;)V", &[JValue::Object(act)])?;
        Ok(())
    });
}

/// 0 = no permission, 1 = waiting, 2 = receiving, 3 = Location switched off.
pub fn gps_state() -> i32 {
    with_gps(|env, cls, act| {
        env.call_static_method(cls, "state", "(Landroid/app/Activity;)I", &[JValue::Object(act)])?
            .i()
    })
    .unwrap_or(0)
}

/// New fixes: (lat, lon, altitude, accuracy m, unix ms).
pub fn gps_drain() -> Vec<[f64; 5]> {
    with_gps(|env, cls, _| {
        let arr = env.call_static_method(cls, "drain", "()[D", &[])?.l()?;
        let arr = JDoubleArray::from(arr);
        let n = env.get_array_length(&arr)? as usize;
        let mut buf = vec![0.0; n];
        env.get_double_array_region(&arr, 0, &mut buf)?;
        Ok(buf.chunks_exact(5).map(|c| [c[0], c[1], c[2], c[3], c[4]]).collect())
    })
    .unwrap_or_default()
}

/// Draws the app behind the status and navigation bars, with bar icons matching the theme.
pub fn edge_to_edge(dark: bool) {
    with_class("id.ghostwalk.app.Screen", |env, cls, act| {
        env.call_static_method(cls, "edgeToEdge", "(Landroid/app/Activity;Z)V", &[JValue::Object(act), JValue::Bool(dark.into())])?;
        Ok(())
    });
}

/// Room taken by the system bars and keyboard, in pixels: left, top, right, bottom, keyboard.
pub fn insets() -> [i32; 5] {
    with_class("id.ghostwalk.app.Screen", |env, cls, act| {
        let arr = env
            .call_static_method(cls, "insets", "(Landroid/app/Activity;)[I", &[JValue::Object(act)])?
            .l()?;
        let arr = JIntArray::from(arr);
        let mut out = [0i32; 5];
        env.get_int_array_region(&arr, 0, &mut out)?;
        Ok(out)
    })
    .unwrap_or_default()
}

/// Opens the phone's file picker; the chosen file arrives later through `picked_file`.
pub fn pick_file() {
    with_class("id.ghostwalk.app.Files", |env, cls, act| {
        env.call_static_method(cls, "pick", "(Landroid/app/Activity;)V", &[JValue::Object(act)])?;
        Ok(())
    });
}

/// The picked file once it is ready: a readable path, `Some("")` when cancelled, `None` while waiting.
pub fn picked_file() -> Option<String> {
    with_class("id.ghostwalk.app.Files", |env, cls, _| {
        let s = env.call_static_method(cls, "take", "()Ljava/lang/String;", &[])?.l()?;
        if s.is_null() {
            return Ok(None);
        }
        let s: String = env.get_string(&s.into())?.into();
        Ok(Some(s))
    })
    .flatten()
}
