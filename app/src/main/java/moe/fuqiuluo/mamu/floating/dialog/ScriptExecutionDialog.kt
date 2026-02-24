package moe.fuqiuluo.mamu.floating.dialog

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.LayoutInflater
import android.widget.TextView
import com.google.android.material.button.MaterialButton
import com.tencent.mmkv.MMKV
import moe.fuqiuluo.mamu.R
import moe.fuqiuluo.mamu.data.settings.getDialogOpacity
import moe.fuqiuluo.mamu.driver.ScriptEngine
import moe.fuqiuluo.mamu.floating.event.FloatingEventBus
import moe.fuqiuluo.mamu.floating.event.SearchResultsUpdatedEvent
import moe.fuqiuluo.mamu.widget.NotificationOverlay
import java.io.File

/**
 * GG-style script execution dialog.
 *
 * Shows file path + "..." browse button + Run/Stop/Cancel.
 * Errors pop up as a separate dialog with Copy / OK.
 * Success is silent.
 */
class ScriptExecutionDialog(
    context: Context,
    private val notification: NotificationOverlay
) : BaseDialog(context) {

    companion object {
        private const val TAG = "ScriptExecutionDialog"
        private const val SCRIPT_DIR_NAME = "mamu/scripts"
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private var selectedFile: File? = null

    private lateinit var tvSelectedFile: TextView
    private lateinit var tvStatus: TextView
    private lateinit var btnRun: MaterialButton
    private lateinit var btnStop: MaterialButton
    private lateinit var btnBrowse: MaterialButton

    override fun setupDialog() {
        val view = LayoutInflater.from(dialog.context)
            .inflate(R.layout.dialog_script_execution, null)
        dialog.setContentView(view)

        // Apply opacity
        val mmkv = MMKV.defaultMMKV()
        val opacity = mmkv.getDialogOpacity()
        view.findViewById<android.view.View>(R.id.root_container)?.background?.alpha =
            (opacity * 255).toInt()

        tvSelectedFile = view.findViewById(R.id.tv_selected_file)
        tvStatus = view.findViewById(R.id.tv_script_status)
        btnRun = view.findViewById(R.id.btn_run_script)
        btnStop = view.findViewById(R.id.btn_stop_script)
        btnBrowse = view.findViewById(R.id.btn_browse)
        val btnCancel: MaterialButton = view.findViewById(R.id.btn_cancel)

        // Browse button
        btnBrowse.setOnClickListener {
            showFilePickerDialog()
        }

        // Run button
        btnRun.setOnClickListener {
            runSelectedScript()
        }

        // Stop button
        btnStop.setOnClickListener {
            try {
                if (ScriptEngine.isRunning()) {
                    ScriptEngine.cancelScript()
                }
            } catch (e: Throwable) {
                Log.e(TAG, "Script cancel failed", e)
            }
        }

        // Cancel button
        btnCancel.setOnClickListener {
            onCancel?.invoke()
            dialog.dismiss()
        }
    }

    private fun showFilePickerDialog() {
        val defaultDir = File(Environment.getExternalStorageDirectory(), SCRIPT_DIR_NAME)
        if (!defaultDir.exists()) defaultDir.mkdirs()

        val picker = ScriptFilePickerDialog(context, defaultDir)
        picker.onFileSelected = { file ->
            selectedFile = file
            tvSelectedFile.text = file.absolutePath
            tvSelectedFile.setTextColor(
                context.resources.getColor(R.color.floating_text_primary, null)
            )
            btnRun.isEnabled = !ScriptEngine.isRunning()
            tvStatus.text = context.getString(R.string.script_status_idle)
        }
        picker.show()
    }

    private fun runSelectedScript() {
        val file = selectedFile
        if (file == null || !file.exists()) {
            notification.showError(context.getString(R.string.script_error_no_file))
            return
        }

        try {
            if (ScriptEngine.isRunning()) {
                notification.showError(context.getString(R.string.script_error_already_running))
                return
            }

            val source = file.readText(Charsets.UTF_8)
            if (source.isBlank()) {
                notification.showError(context.getString(R.string.script_error_empty))
                return
            }

            ScriptEngine.reset()

            val callback = object : ScriptEngine.Callback {
                override fun onLog(message: String) {
                    // Script print() — no UI output
                }

                override fun onToast(message: String) {
                    mainHandler.post { notification.showSuccess(message) }
                }

                override fun onFinished(status: Int, errorMessage: String?) {
                    mainHandler.post {
                        when (status) {
                            ScriptEngine.Status.COMPLETED -> {
                                onScriptFinished(R.string.script_status_completed)
                            }
                            ScriptEngine.Status.CANCELLED -> {
                                onScriptFinished(R.string.script_status_cancelled)
                            }
                            ScriptEngine.Status.ERROR -> {
                                onScriptFinished(R.string.script_status_error)
                                if (!errorMessage.isNullOrEmpty()) {
                                    showErrorDialog(errorMessage)
                                }
                            }
                            else -> onScriptFinished(R.string.script_status_idle)
                        }
                    }
                }

                override fun onSearchCompleted(count: Long) {
                    FloatingEventBus.tryEmitSearchResultsUpdated(
                        SearchResultsUpdatedEvent(
                            totalCount = count,
                            ranges = emptyList()
                        )
                    )
                }
            }

            val started = ScriptEngine.executeScript(source, file.name, callback)
            if (started) {
                onScriptStarted()
            } else {
                notification.showError(context.getString(R.string.script_error_already_running))
            }
        } catch (e: Throwable) {
            Log.e(TAG, "Script execution failed", e)
            onScriptFinished(R.string.script_status_error)
            showErrorDialog("${e.javaClass.simpleName}: ${e.message}")
        }
    }

    private fun onScriptStarted() {
        tvStatus.text = context.getString(R.string.script_status_running)
        btnRun.isEnabled = false
        btnStop.isEnabled = true
        btnBrowse.isEnabled = false
    }

    private fun onScriptFinished(statusResId: Int) {
        tvStatus.text = context.getString(statusResId)
        btnRun.isEnabled = selectedFile != null
        btnStop.isEnabled = false
        btnBrowse.isEnabled = true
    }

    private fun showErrorDialog(errorText: String) {
        val errorDialog = object : BaseDialog(context) {
            override fun setupDialog() {
                val v = LayoutInflater.from(dialog.context)
                    .inflate(R.layout.dialog_script_error, null)
                dialog.setContentView(v)

                val tvError = v.findViewById<TextView>(R.id.tv_error_message)
                val btnCopy = v.findViewById<MaterialButton>(R.id.btn_copy)
                val btnOk = v.findViewById<MaterialButton>(R.id.btn_ok)

                tvError.text = errorText

                btnCopy.setOnClickListener {
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE)
                            as ClipboardManager
                    clipboard.setPrimaryClip(
                        ClipData.newPlainText("script_error", errorText)
                    )
                    notification.showSuccess("已复制")
                }

                btnOk.setOnClickListener {
                    dialog.dismiss()
                }
            }
        }
        errorDialog.show()
    }

    override fun dismiss() {
        // Cancel running script on dismiss
        try {
            if (ScriptEngine.isRunning()) {
                ScriptEngine.cancelScript()
            }
        } catch (_: Throwable) {}
        super.dismiss()
    }
}
