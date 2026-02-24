//! JNI methods for the Lua script engine.
//!
//! Exposes script lifecycle operations (execute, cancel, status) to Kotlin
//! via the `ScriptEngine` JNI facade class.

use crate::script::runtime::{ScriptCallback, ScriptRuntime, ScriptStatus};
use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::sys::{jboolean, jint, JNI_FALSE, JNI_TRUE};
use jni::{JNIEnv, JavaVM};
use jni_macro::jni_method;
use log::{error, info};
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;

lazy_static! {
    /// Global script runtime instance. Only one script can run at a time.
    static ref SCRIPT_RUNTIME: Mutex<ScriptRuntime> = Mutex::new(ScriptRuntime::new());
}

// ---------------------------------------------------------------------------
// JNI callback implementation
// ---------------------------------------------------------------------------

/// Bridges Rust script callbacks to Kotlin methods via JNI.
struct JniScriptCallback {
    vm: JavaVM,
    callback: GlobalRef,
}

unsafe impl Send for JniScriptCallback {}
unsafe impl Sync for JniScriptCallback {}

impl ScriptCallback for JniScriptCallback {
    fn on_log(&self, message: &str) {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let jmsg = match env.new_string(message) {
                Ok(s) => s,
                Err(_) => return,
            };
            let _ = env.call_method(
                &self.callback,
                "onLog",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&jmsg)],
            );
        }
    }

    fn on_toast(&self, message: &str) {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let jmsg = match env.new_string(message) {
                Ok(s) => s,
                Err(_) => return,
            };
            let _ = env.call_method(
                &self.callback,
                "onToast",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&jmsg)],
            );
        }
    }

    fn on_finished(&self, status: ScriptStatus, error_message: Option<&str>) {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let status_int = status as u8 as jint;
            let jerror = match error_message {
                Some(msg) => match env.new_string(msg) {
                    Ok(s) => JObject::from(s),
                    Err(_) => JObject::null(),
                },
                None => JObject::null(),
            };
            let _ = env.call_method(
                &self.callback,
                "onFinished",
                "(ILjava/lang/String;)V",
                &[JValue::Int(status_int), JValue::Object(&jerror)],
            );
        }
    }

    fn on_search_completed(&self, count: i64) {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let _ = env.call_method(
                &self.callback,
                "onSearchCompleted",
                "(J)V",
                &[JValue::Long(count)],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// JNI methods
// ---------------------------------------------------------------------------

/// Execute a Lua script from source code.
///
/// Kotlin signature:
/// ```kotlin
/// external fun nativeExecuteScript(source: String, name: String, callback: Any): Boolean
/// ```
#[jni_method(70, "moe/fuqiuluo/mamu/driver/ScriptEngine", "nativeExecuteScript", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/Object;)Z")]
pub fn jni_execute_script(
    mut env: JNIEnv,
    _clazz: JObject,
    source: JString,
    name: JString,
    callback: JObject,
) -> jboolean {
    let source: String = match env.get_string(&source) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get script source string: {}", e);
            return JNI_FALSE;
        }
    };

    let name: String = match env.get_string(&name) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get script name string: {}", e);
            return JNI_FALSE;
        }
    };

    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            error!("Failed to get JavaVM: {}", e);
            return JNI_FALSE;
        }
    };

    let global_callback = match env.new_global_ref(callback) {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to create global ref for callback: {}", e);
            return JNI_FALSE;
        }
    };

    let jni_callback = Arc::new(JniScriptCallback {
        vm,
        callback: global_callback,
    });

    let mut runtime = match SCRIPT_RUNTIME.lock() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to lock script runtime: {}", e);
            return JNI_FALSE;
        }
    };

    match runtime.execute(source, name, jni_callback) {
        Ok(()) => {
            info!("Script execution started");
            JNI_TRUE
        }
        Err(e) => {
            error!("Failed to start script: {}", e);
            JNI_FALSE
        }
    }
}

/// Cancel the currently running script.
///
/// Kotlin signature:
/// ```kotlin
/// external fun nativeCancelScript()
/// ```
#[jni_method(70, "moe/fuqiuluo/mamu/driver/ScriptEngine", "nativeCancelScript", "()V")]
pub fn jni_cancel_script(
    _env: JNIEnv,
    _clazz: JObject,
) {
    if let Ok(mut runtime) = SCRIPT_RUNTIME.lock() {
        runtime.cancel();
        info!("Script cancellation requested via JNI");
    }
}

/// Get the current script execution status.
///
/// Returns: 0=Idle, 1=Running, 2=Completed, 3=Cancelled, 4=Error
///
/// Kotlin signature:
/// ```kotlin
/// external fun nativeGetScriptStatus(): Int
/// ```
#[jni_method(70, "moe/fuqiuluo/mamu/driver/ScriptEngine", "nativeGetScriptStatus", "()I")]
pub fn jni_get_script_status(
    _env: JNIEnv,
    _clazz: JObject,
) -> jint {
    match SCRIPT_RUNTIME.lock() {
        Ok(runtime) => runtime.status() as u8 as jint,
        Err(_) => ScriptStatus::Idle as u8 as jint,
    }
}

/// Check if a script is currently running.
///
/// Kotlin signature:
/// ```kotlin
/// external fun nativeIsScriptRunning(): Boolean
/// ```
#[jni_method(70, "moe/fuqiuluo/mamu/driver/ScriptEngine", "nativeIsScriptRunning", "()Z")]
pub fn jni_is_script_running(
    _env: JNIEnv,
    _clazz: JObject,
) -> jboolean {
    match SCRIPT_RUNTIME.lock() {
        Ok(runtime) => {
            if runtime.is_running() { JNI_TRUE } else { JNI_FALSE }
        }
        Err(_) => JNI_FALSE,
    }
}

/// Reset the script runtime to idle state.
///
/// Kotlin signature:
/// ```kotlin
/// external fun nativeResetScript()
/// ```
#[jni_method(70, "moe/fuqiuluo/mamu/driver/ScriptEngine", "nativeResetScript", "()V")]
pub fn jni_reset_script(
    _env: JNIEnv,
    _clazz: JObject,
) {
    if let Ok(mut runtime) = SCRIPT_RUNTIME.lock() {
        runtime.reset();
    }
}
