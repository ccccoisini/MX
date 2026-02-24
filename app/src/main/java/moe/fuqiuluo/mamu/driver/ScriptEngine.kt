@file:Suppress("KotlinJniMissingFunction")

package moe.fuqiuluo.mamu.driver

/**
 * JNI facade for the Rust Lua script engine.
 *
 * Provides script lifecycle management: execute, cancel, query status.
 * Only one script can run at a time.
 */
object ScriptEngine {

    init {
        System.loadLibrary("mamu_core")
    }

    /** Script execution status constants (matches Rust ScriptStatus). */
    object Status {
        const val IDLE = 0
        const val RUNNING = 1
        const val COMPLETED = 2
        const val CANCELLED = 3
        const val ERROR = 4
    }

    /**
     * Callback interface for script engine events.
     * Methods are called from the Rust script execution thread.
     */
    interface Callback {
        /** Append a log line to the output panel. */
        fun onLog(message: String)

        /** Show a short toast notification. */
        fun onToast(message: String)

        /** Called when script execution finishes. */
        fun onFinished(status: Int, errorMessage: String?)

        /** Called when a script-initiated search completes, so the UI can update. */
        fun onSearchCompleted(count: Long)
    }

    /**
     * Execute a Lua script from source code.
     *
     * @param source   The Lua source code to execute.
     * @param name     A display name for the script (used in logs/errors).
     * @param callback Callback for log, toast, and completion events.
     * @return true if the script was successfully started.
     */
    fun executeScript(source: String, name: String, callback: Callback): Boolean {
        return nativeExecuteScript(source, name, callback)
    }

    /** Request cancellation of the running script. */
    fun cancelScript() = nativeCancelScript()

    /** Get current execution status (see [Status] constants). */
    fun getStatus(): Int = nativeGetScriptStatus()

    /** Check if a script is currently running. */
    fun isRunning(): Boolean = nativeIsScriptRunning()

    /** Reset the runtime to idle state after completion/error/cancel. */
    fun reset() = nativeResetScript()

    // -- Native methods --

    private external fun nativeExecuteScript(source: String, name: String, callback: Any): Boolean
    private external fun nativeCancelScript()
    private external fun nativeGetScriptStatus(): Int
    private external fun nativeIsScriptRunning(): Boolean
    private external fun nativeResetScript()
}
