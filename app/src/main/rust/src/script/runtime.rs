//! Script execution runtime with lifecycle management.
//!
//! [`ScriptRuntime`] coordinates script execution on a background thread,
//! provides cancellation support, and manages the engine lifecycle.

use anyhow::Result;
use log::{error, info, warn};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;

use crate::script::engine::ScriptEngine;

// ---------------------------------------------------------------------------
// Script status
// ---------------------------------------------------------------------------

/// Execution status of the script runtime.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptStatus {
    /// No script is running.
    Idle = 0,
    /// A script is currently executing.
    Running = 1,
    /// The last script finished successfully.
    Completed = 2,
    /// The running script was cancelled by the user.
    Cancelled = 3,
    /// The last script terminated with an error.
    Error = 4,
}

impl ScriptStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Running,
            2 => Self::Completed,
            3 => Self::Cancelled,
            4 => Self::Error,
            _ => Self::Idle,
        }
    }
}

// ---------------------------------------------------------------------------
// Script callback trait
// ---------------------------------------------------------------------------

/// Callback interface from the script runtime back to the host (Kotlin/JNI).
///
/// Implementations must be `Send + Sync` because callbacks fire from the
/// script execution thread.
pub trait ScriptCallback: Send + Sync {
    /// Append a log line to the UI log panel.
    fn on_log(&self, message: &str);

    /// Show a short toast notification to the user.
    fn on_toast(&self, message: &str);

    /// Called when script execution finishes (success, error, or cancel).
    fn on_finished(&self, status: ScriptStatus, error_message: Option<&str>);

    /// Called when a script-initiated search completes, so the UI can update.
    fn on_search_completed(&self, count: i64);
}

// ---------------------------------------------------------------------------
// Script runtime
// ---------------------------------------------------------------------------

/// Manages script execution lifecycle including start, cancel, and status.
///
/// Only one script can run at a time. Attempting to start a second script
/// while one is already running will return an error.
pub struct ScriptRuntime {
    /// Current execution status (atomic for lock-free reads from any thread).
    status: Arc<AtomicU8>,

    /// Cancellation flag checked by long-running API calls (e.g. `mamu.sleep`).
    cancel_flag: Arc<AtomicU8>,

    /// Handle to the execution thread (if any).
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl ScriptRuntime {
    /// Create a new idle runtime.
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(ScriptStatus::Idle as u8)),
            cancel_flag: Arc::new(AtomicU8::new(0)),
            thread_handle: None,
        }
    }

    /// Current execution status.
    pub fn status(&self) -> ScriptStatus {
        ScriptStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    /// Whether a script is currently running.
    pub fn is_running(&self) -> bool {
        self.status() == ScriptStatus::Running
    }

    /// Get a clone of the cancel flag for use in API functions.
    pub fn cancel_flag(&self) -> Arc<AtomicU8> {
        self.cancel_flag.clone()
    }

    /// Execute a script asynchronously on a background thread.
    ///
    /// # Errors
    ///
    /// Returns an error if a script is already running.
    pub fn execute(
        &mut self,
        source: String,
        script_name: String,
        callback: Arc<dyn ScriptCallback>,
    ) -> Result<()> {
        if self.is_running() {
            anyhow::bail!("A script is already running");
        }

        // Clean up previous thread handle
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        // Reset state
        self.status.store(ScriptStatus::Running as u8, Ordering::SeqCst);
        self.cancel_flag.store(0, Ordering::SeqCst);

        let status = self.status.clone();
        let cancel_flag = self.cancel_flag.clone();
        let cb = callback.clone();

        let handle = thread::Builder::new()
            .name("mamu-script".into())
            .spawn(move || {
                info!("Script thread started: {}", script_name);
                cb.on_log(&format!("[系统] 开始执行脚本: {}", script_name));

                // Create a fresh engine for this execution
                let engine = match ScriptEngine::new(callback.clone()) {
                    Ok(e) => e,
                    Err(e) => {
                        let msg = format!("引擎初始化失败: {}", e);
                        error!("{}", msg);
                        cb.on_log(&format!("[错误] {}", msg));
                        cb.on_finished(ScriptStatus::Error, Some(&msg));
                        status.store(ScriptStatus::Error as u8, Ordering::SeqCst);
                        return;
                    }
                };

                // Inject the cancel flag into Lua registry for API functions to read
                if let Err(e) = inject_cancel_flag(&engine, cancel_flag.clone()) {
                    let msg = format!("内部错误: {}", e);
                    error!("{}", msg);
                    cb.on_log(&format!("[错误] {}", msg));
                    cb.on_finished(ScriptStatus::Error, Some(&msg));
                    status.store(ScriptStatus::Error as u8, Ordering::SeqCst);
                    return;
                }

                // Execute the script
                match engine.execute(&source, &script_name) {
                    Ok(()) => {
                        // Check if we were cancelled during execution
                        if cancel_flag.load(Ordering::Relaxed) != 0 {
                            info!("Script was cancelled: {}", script_name);
                            cb.on_log("[系统] 脚本已取消");
                            cb.on_finished(ScriptStatus::Cancelled, None);
                            status.store(ScriptStatus::Cancelled as u8, Ordering::SeqCst);
                        } else {
                            info!("Script completed: {}", script_name);
                            cb.on_log("[系统] 脚本执行完成");
                            cb.on_finished(ScriptStatus::Completed, None);
                            status.store(ScriptStatus::Completed as u8, Ordering::SeqCst);
                        }
                    }
                    Err(e) => {
                        if cancel_flag.load(Ordering::Relaxed) != 0 {
                            warn!("Script cancelled with error: {}", e);
                            cb.on_log("[系统] 脚本已取消");
                            cb.on_finished(ScriptStatus::Cancelled, None);
                            status.store(ScriptStatus::Cancelled as u8, Ordering::SeqCst);
                        } else {
                            let msg = format!("{}", e);
                            error!("Script error: {}", msg);
                            cb.on_log(&format!("[错误] {}", msg));
                            cb.on_finished(ScriptStatus::Error, Some(&msg));
                            status.store(ScriptStatus::Error as u8, Ordering::SeqCst);
                        }
                    }
                }
            })?;

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Request cancellation of the running script.
    ///
    /// This sets the cancel flag which is checked by blocking API calls
    /// like `mamu.sleep`. The script will stop at the next check point.
    pub fn cancel(&mut self) {
        if self.is_running() {
            info!("Cancellation requested");
            self.cancel_flag.store(1, Ordering::SeqCst);
        }
    }

    /// Reset the runtime to idle state. Call after retrieving final status.
    pub fn reset(&mut self) {
        if !self.is_running() {
            self.status.store(ScriptStatus::Idle as u8, Ordering::SeqCst);
            self.cancel_flag.store(0, Ordering::SeqCst);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Store the cancel flag in the Lua registry so API functions can read it.
fn inject_cancel_flag(_engine: &ScriptEngine, flag: Arc<AtomicU8>) -> Result<()> {
    // The engine exposes the Lua instance indirectly through execute().
    // We store the flag in a global static that API functions read.
    // This is safe because only one script runs at a time.
    CANCEL_FLAG.store_flag(flag);
    Ok(())
}

/// Global cancel flag accessor for API functions.
///
/// Only one script runs at a time, so a single global is safe.
pub(crate) struct CancelFlagHolder {
    inner: std::sync::RwLock<Option<Arc<AtomicU8>>>,
}

impl CancelFlagHolder {
    const fn new() -> Self {
        Self {
            inner: std::sync::RwLock::new(None),
        }
    }

    pub fn store_flag(&self, flag: Arc<AtomicU8>) {
        *self.inner.write().unwrap() = Some(flag);
    }

    /// Check whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|f| f.load(Ordering::Relaxed) != 0))
            .unwrap_or(false)
    }
}

pub(crate) static CANCEL_FLAG: CancelFlagHolder = CancelFlagHolder::new();
